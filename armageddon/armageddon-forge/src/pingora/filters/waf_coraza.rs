// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Coraza-on-Pingora WAF filter (proxy-wasm v0.2.1 host).
//!
//! Compiled only when the `coraza-wasm` Cargo feature is enabled.  In
//! default builds this file is absent and the regex WAF
//! (`filters::waf::WafFilter`) is the only WAF in the binary.
//!
//! # Status — scaffold
//!
//! The filter loads and instantiates the Coraza module successfully but
//! does **not** yet inspect traffic — `CorazaInstance::on_request_body`
//! is a stub that returns `Decision::Continue`.  The full host-function
//! bring-up roadmap lives in
//! `armageddon/coraza/PROXY-WASM-HOST-DESIGN.md` (~10 h of work).
//!
//! # Wiring
//!
//! When the binary boots with `coraza-wasm` enabled AND
//! `gateway.waf.wasm_module` is set, `armageddon::main` constructs this
//! filter in place of the regex WAF.  If module loading fails:
//!
//! * `fail_closed = true` → boot error (production posture).
//! * `fail_closed = false` → log and fall back to the regex WAF.

use std::path::PathBuf;
use std::sync::Arc;

use prometheus::{IntCounterVec, Opts, Registry};
use tracing::{debug, error, warn};

use armageddon_wasm::proxy_wasm_v0_2_1::{
    CorazaHostError, CorazaModule, Decision as CorazaDecision,
};

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Runtime configuration mirror — decoupled from
/// `armageddon_config::WafConfig` to avoid pulling the config crate
/// into the forge crate.
#[derive(Debug, Clone)]
pub struct WafCorazaConfig {
    /// Master switch.
    pub enabled: bool,
    /// Path to the compiled `coraza-waf.wasm`.
    pub wasm_module_path: PathBuf,
    /// Optional path to `coraza.conf` (CRS rules).  When set, the file
    /// is read at boot and exposed to the guest via
    /// `proxy_on_configure`.  When unset, Coraza falls back to its
    /// built-in defaults.
    pub coraza_conf_path: Option<PathBuf>,
    /// HTTP status returned on a Coraza-induced block (default 403).
    pub block_status: u16,
    /// When true, log matches but pass the request through.
    pub learning_mode: bool,
    /// When true, treat module-load errors as fatal at startup.
    pub fail_closed_on_load_error: bool,
    /// Body inspection cap (mirrors regex WAF default — 256 KiB).
    pub max_body_inspect_bytes: usize,
}

impl Default for WafCorazaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            wasm_module_path: PathBuf::new(),
            coraza_conf_path: None,
            block_status: 403,
            learning_mode: false,
            fail_closed_on_load_error: true,
            max_body_inspect_bytes: 256 * 1024,
        }
    }
}

/// Prometheus metrics for the Coraza WAF.
///
/// Names are namespaced with `_coraza` suffix so they don't clash with
/// the regex WAF metrics — this lets shadow-mode comparison expose both
/// counters simultaneously.
#[derive(Debug, Clone)]
struct WafCorazaMetrics {
    blocks_total: IntCounterVec,
    evaluations_total: IntCounterVec,
}

impl WafCorazaMetrics {
    fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let blocks_total = IntCounterVec::new(
            Opts::new(
                "armageddon_waf_coraza_blocks_total",
                "Requests blocked by Coraza WAF, labelled by status code",
            ),
            &["status"],
        )?;
        registry.register(Box::new(blocks_total.clone()))?;

        let evaluations_total = IntCounterVec::new(
            Opts::new(
                "armageddon_waf_coraza_evaluations_total",
                "Total Coraza evaluations, labelled by outcome (allow|block|learning_log|skipped)",
            ),
            &["outcome"],
        )?;
        registry.register(Box::new(evaluations_total.clone()))?;

        Ok(Self {
            blocks_total,
            evaluations_total,
        })
    }
}

/// Coraza-backed WAF filter.
///
/// The compiled `CorazaModule` is shared (Arc) across requests; each
/// request creates a fresh `CorazaInstance` (per-request `Store`).  All
/// per-request state (header snapshot, body buffer) lives in the
/// per-request `RequestCtx`, never on the filter itself — the filter is
/// shared as `Arc<dyn ForgeFilter>` and must remain stateless.
pub struct WafCorazaFilter {
    module: Arc<CorazaModule>,
    config: WafCorazaConfig,
    metrics: WafCorazaMetrics,
}

impl std::fmt::Debug for WafCorazaFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WafCorazaFilter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WafCorazaFilter {
    /// Build a filter by loading + AOT-compiling the Coraza module.
    ///
    /// Returns an error if module compilation fails — callers map this
    /// to either a fatal boot error (`fail_closed_on_load_error = true`)
    /// or a log-and-fallback to the regex WAF.
    pub fn new(config: WafCorazaConfig, registry: &Registry) -> Result<Self, WafCorazaError> {
        let module = match config.coraza_conf_path.as_deref() {
            Some(conf_path) => CorazaModule::load_with_config(&config.wasm_module_path, conf_path)
                .map_err(WafCorazaError::Load)?,
            None => CorazaModule::load(&config.wasm_module_path).map_err(WafCorazaError::Load)?,
        };
        let metrics = WafCorazaMetrics::new(registry)
            .map_err(|e| WafCorazaError::MetricsRegister(e.to_string()))?;
        Ok(Self {
            module: Arc::new(module),
            config,
            metrics,
        })
    }
}

/// Errors emitted when constructing a Coraza WAF filter.
#[derive(Debug, thiserror::Error)]
pub enum WafCorazaError {
    #[error("coraza module load failed: {0}")]
    Load(#[source] CorazaHostError),
    #[error("metrics registration failed: {0}")]
    MetricsRegister(String),
}

#[async_trait::async_trait]
impl ForgeFilter for WafCorazaFilter {
    fn name(&self) -> &'static str {
        "waf-coraza"
    }

    /// Headers-phase Coraza dispatch.
    ///
    /// Captures method/path/headers/source_addr into `RequestCtx` so the
    /// body-phase hook can dispatch them together to a fresh
    /// `CorazaInstance` (Coraza's per-request transaction state requires
    /// running headers + body on the same instance, but the instance is
    /// `!Send` so we cannot persist it across `async` await points).
    ///
    /// `RequestCtx` already stores `http_method`, `http_path`,
    /// `http_headers` — populated by the upstream `request_filter` core
    /// hook — so this method is mostly a no-op verifying the data is
    /// there.  The actual block decision happens at end-of-stream.
    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if !self.config.enabled {
            return Decision::Continue;
        }
        // Defensive: re-populate ctx fields if the upstream hook hasn't
        // — this filter must work whether or not the core
        // request_filter ran first.
        if ctx.http_method.is_none() {
            ctx.http_method = Some(session.req_header().method.to_string());
        }
        if ctx.http_path.is_none() {
            let req = session.req_header();
            let path = req
                .uri
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| req.uri.path().to_string());
            ctx.http_path = Some(path);
        }
        if ctx.http_headers.is_empty() {
            for (name, value) in session.req_header().headers.iter() {
                if let Ok(v) = value.to_str() {
                    ctx.http_headers
                        .insert(name.as_str().to_lowercase(), v.to_string());
                }
            }
        }
        debug!(
            request_id = %ctx.request_id,
            method = ?ctx.http_method,
            path = ?ctx.http_path,
            headers = ctx.http_headers.len(),
            "waf-coraza: snapshot captured for end-of-stream dispatch",
        );
        Decision::Continue
    }

    /// Body-phase Coraza dispatch.
    ///
    /// Buffers chunks into `ctx.body_buffer` (capped); on `end_of_stream`
    /// instantiates a fresh `CorazaInstance`, runs
    /// `on_request_headers` then `on_request_body` and maps the verdict
    /// onto the forge `Decision`.
    async fn on_request_body(
        &self,
        session: &mut pingora_proxy::Session,
        body: &Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if !self.config.enabled {
            return Decision::Continue;
        }

        // Accumulate into the per-request ctx buffer (capped).
        if let Some(chunk) = body.as_ref() {
            let cap = self.config.max_body_inspect_bytes;
            let remaining = cap.saturating_sub(ctx.body_buffer.len());
            if remaining == 0 {
                ctx.body_buffer_overflow = true;
            } else {
                let take = remaining.min(chunk.len());
                ctx.body_buffer.extend_from_slice(&chunk[..take]);
                if take < chunk.len() {
                    ctx.body_buffer_overflow = true;
                }
            }
        }

        if !end_of_stream {
            return Decision::Continue;
        }

        // Source address from the Pingora session, best-effort.
        let source_addr = session
            .client_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();

        let method = ctx.http_method.clone().unwrap_or_else(|| "GET".to_string());
        let path = ctx.http_path.clone().unwrap_or_else(|| "/".to_string());
        let headers: Vec<(String, String)> = ctx
            .http_headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let body_bytes = std::mem::take(&mut ctx.body_buffer);

        // Instantiate per-request Coraza state.
        let mut instance = match self.module.create_instance() {
            Ok(i) => i,
            Err(e) => {
                error!(
                    err = %e,
                    request_id = %ctx.request_id,
                    "waf-coraza: instance creation failed; failing closed (503)",
                );
                self.metrics
                    .evaluations_total
                    .with_label_values(&["skipped"])
                    .inc();
                // Fail closed on instance failure — the WAF is a security
                // control and bypass-on-error would defeat the purpose.
                return Decision::Deny(503);
            }
        };

        // Headers phase.
        let header_verdict = instance.on_request_headers(&method, &path, &source_addr, &headers);
        if let CorazaDecision::Deny { status, reason } = header_verdict {
            return self.map_verdict(ctx, status, reason);
        }

        // Body phase.
        let body_verdict = instance.on_request_body(&body_bytes);
        match body_verdict {
            CorazaDecision::Continue => {
                self.metrics
                    .evaluations_total
                    .with_label_values(&["allow"])
                    .inc();
                Decision::Continue
            }
            CorazaDecision::Deny { status, reason } => self.map_verdict(ctx, status, reason),
        }
    }
}

impl WafCorazaFilter {
    /// Translate a Coraza Deny into the forge `Decision`, honouring
    /// learning_mode and emitting metrics.
    fn map_verdict(&self, ctx: &RequestCtx, status: u16, reason: String) -> Decision {
        let status = if status == 0 {
            self.config.block_status
        } else {
            status
        };

        if self.config.learning_mode {
            self.metrics
                .evaluations_total
                .with_label_values(&["learning_log"])
                .inc();
            warn!(
                request_id = %ctx.request_id,
                status,
                reason = %reason,
                "waf-coraza: rule matched (learning mode — pass-through)",
            );
            Decision::Continue
        } else {
            self.metrics
                .blocks_total
                .with_label_values(&[&status.to_string()])
                .inc();
            self.metrics
                .evaluations_total
                .with_label_values(&["block"])
                .inc();
            warn!(
                request_id = %ctx.request_id,
                status,
                reason = %reason,
                "waf-coraza: blocked",
            );
            Decision::Deny(status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: configuration default is sensible.
    #[test]
    fn default_config_is_fail_closed_403() {
        let cfg = WafCorazaConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.block_status, 403);
        assert!(cfg.fail_closed_on_load_error);
        assert!(!cfg.learning_mode);
    }

    /// Loading a missing path returns a Load error.
    #[test]
    fn new_with_missing_module_returns_load_error() {
        let registry = Registry::new();
        let mut cfg = WafCorazaConfig::default();
        cfg.wasm_module_path = PathBuf::from("/nonexistent/coraza.wasm");
        let result = WafCorazaFilter::new(cfg, &registry);
        assert!(
            matches!(result, Err(WafCorazaError::Load(_))),
            "expected Load error, got {result:?}",
        );
    }
}
