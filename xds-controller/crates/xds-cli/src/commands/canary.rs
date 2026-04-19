// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! CLI handler for `xdsctl canary` subcommands.
//!
//! Communicates with the xDS Controller's CanaryService gRPC endpoint.
//!
//! # Usage
//!
//! ```text
//! xdsctl canary start  --service poulets-api --image-tag v2.3.1 \
//!                      --prometheus http://prometheus:9090 \
//!                      --error-rate-max 0.005 --latency-p99-max 50
//! xdsctl canary status --canary-id <uuid>
//! xdsctl canary pause  --canary-id <uuid>
//! xdsctl canary abort  --canary-id <uuid> --reason "manual rollback"
//! xdsctl canary promote --canary-id <uuid>
//! xdsctl canary list   --service poulets-api
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use tonic::transport::Channel;

// The generated client lives in xds-server. For the CLI we re-export the
// generated types via a local path re-use. In practice the CLI crate depends
// on xds-server; we import the client module directly.
use xds_server::generated::canary::v1::{
    canary_service_client::CanaryServiceClient, AbortCanaryRequest, GetCanaryStatusRequest,
    ListCanariesRequest, PauseCanaryRequest, PromoteCanaryRequest, SloConfig,
    StartCanaryRequest,
};

/// CLI sub-commands for canary management.
#[derive(Subcommand, Debug)]
pub enum CanaryAction {
    /// Start a new canary deployment (1 % → 10 % → 50 % → 100 %).
    Start {
        /// Service name (must match a cluster `<service>-stable` in the store).
        #[arg(long)]
        service: String,

        /// Docker image tag for the canary build.
        #[arg(long)]
        image_tag: String,

        /// Prometheus endpoint for SLO polling.
        #[arg(long, default_value = "http://prometheus:9090")]
        prometheus: String,

        /// Maximum acceptable error rate (0.0–1.0).
        #[arg(long, default_value = "0.005")]
        error_rate_max: f64,

        /// Maximum acceptable p99 latency in milliseconds.
        #[arg(long, default_value = "50")]
        latency_p99_max: f64,

        /// Minimum duration (seconds) each stage must hold before auto-advance.
        #[arg(long, default_value = "3600")]
        min_stage_secs: u64,
    },

    /// Show status of a running canary.
    Status {
        /// Canary ID returned by `start`.
        #[arg(long)]
        canary_id: String,
    },

    /// Pause tick-driven advancement (route weights unchanged).
    Pause {
        #[arg(long)]
        canary_id: String,
    },

    /// Abort a canary and roll back to 0 % immediately.
    Abort {
        #[arg(long)]
        canary_id: String,
        #[arg(long, default_value = "manual abort via CLI")]
        reason: String,
    },

    /// Force-promote a canary to 100 % regardless of SLO.
    Promote {
        #[arg(long)]
        canary_id: String,
    },

    /// List all canaries for a service.
    List {
        #[arg(long)]
        service: String,
    },
}

/// Entry point called from `main.rs`.
pub async fn handle(action: CanaryAction, server: &str) -> Result<()> {
    let channel = Channel::from_shared(server.to_string())
        .context("invalid server address")?
        .connect()
        .await
        .with_context(|| format!("could not connect to xDS Controller at {server}"))?;

    let mut client = CanaryServiceClient::new(channel);

    match action {
        CanaryAction::Start {
            service,
            image_tag,
            prometheus,
            error_rate_max,
            latency_p99_max,
            min_stage_secs,
        } => {
            let req = StartCanaryRequest {
                service: service.clone(),
                image_tag: image_tag.clone(),
                stages: vec![1, 10, 50, 100],
                min_stage_duration_secs: min_stage_secs,
                slo: Some(SloConfig {
                    error_rate_max,
                    latency_p99_max_ms: latency_p99_max,
                    prometheus_endpoint: prometheus,
                }),
            };
            let resp = client
                .start_canary(req)
                .await
                .context("StartCanary RPC failed")?;

            let canary_id = resp.into_inner().canary_id;
            println!("Canary started:");
            println!("  service   : {service}");
            println!("  image_tag : {image_tag}");
            println!("  canary_id : {canary_id}");
            println!("  stage     : 1 %");
        }

        CanaryAction::Status { canary_id } => {
            let resp = client
                .get_canary_status(GetCanaryStatusRequest {
                    canary_id: canary_id.clone(),
                })
                .await
                .context("GetCanaryStatus RPC failed")?;

            print_status(&resp.into_inner());
        }

        CanaryAction::Pause { canary_id } => {
            let resp = client
                .pause_canary(PauseCanaryRequest {
                    canary_id: canary_id.clone(),
                })
                .await
                .context("PauseCanary RPC failed")?;

            if let Some(status) = resp.into_inner().status {
                println!("Canary paused:");
                print_status(&status);
            }
        }

        CanaryAction::Abort { canary_id, reason } => {
            let resp = client
                .abort_canary(AbortCanaryRequest {
                    canary_id: canary_id.clone(),
                    reason: reason.clone(),
                })
                .await
                .context("AbortCanary RPC failed")?;

            if let Some(status) = resp.into_inner().status {
                println!("Canary aborted (reason: {reason}):");
                print_status(&status);
            }
        }

        CanaryAction::Promote { canary_id } => {
            let resp = client
                .promote_canary(PromoteCanaryRequest {
                    canary_id: canary_id.clone(),
                })
                .await
                .context("PromoteCanary RPC failed")?;

            if let Some(status) = resp.into_inner().status {
                println!("Canary promoted to 100 %:");
                print_status(&status);
            }
        }

        CanaryAction::List { service } => {
            let resp = client
                .list_canaries(ListCanariesRequest {
                    service: service.clone(),
                    stage_filter: None,
                })
                .await
                .context("ListCanaries RPC failed")?;

            let canaries = resp.into_inner().canaries;
            if canaries.is_empty() {
                println!("No canaries found for service '{service}'.");
            } else {
                println!("Canaries for service '{service}':");
                for c in &canaries {
                    print_status(c);
                    println!("---");
                }
            }
        }
    }

    Ok(())
}

fn print_status(s: &xds_server::generated::canary::v1::CanaryStatus) {
    println!("  canary_id    : {}", s.canary_id);
    println!("  service      : {}", s.service);
    println!("  image_tag    : {}", s.image_tag);
    println!("  stage        : {}", s.current_stage);
    println!("  weight       : {} %", s.current_weight_pct);
    if !s.rollback_reason.is_empty() {
        println!("  rollback_reason: {}", s.rollback_reason);
    }
    if let Some(c) = &s.slo_compliance {
        println!(
            "  slo          : error_rate={:.4} p99={:.1}ms within_budget={}",
            c.observed_error_rate, c.observed_latency_p99_ms, c.within_budget
        );
    }
}
