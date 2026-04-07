//! Pentagon pipeline: initializes and orchestrates all security engines.

use anyhow::Result;
use armageddon_aegis::Aegis;
use armageddon_ai::AiEngine;
use armageddon_arbiter::Arbiter;
use armageddon_common::context::RequestContext;
use armageddon_common::decision::Decision;
use armageddon_common::engine::SecurityEngine;
use armageddon_config::ArmageddonConfig;
use armageddon_nexus::{FinalVerdict, Nexus};
use armageddon_oracle::Oracle;
use armageddon_sentinel::Sentinel;
use armageddon_veil::Veil;
use armageddon_wasm::WasmRuntime;

/// The Pentagon: 5 security engines + NEXUS brain + VEIL + WASM.
pub struct Pentagon {
    sentinel: Sentinel,
    arbiter: Arbiter,
    oracle: Oracle,
    aegis: Aegis,
    ai: AiEngine,
    nexus: Nexus,
    veil: Veil,
    wasm: WasmRuntime,
}

impl Pentagon {
    /// Create the Pentagon from configuration.
    pub fn new(config: &ArmageddonConfig) -> Result<Self> {
        let sec = &config.security;

        Ok(Self {
            sentinel: Sentinel::new(sec.sentinel.clone()),
            arbiter: Arbiter::new(sec.arbiter.clone()),
            oracle: Oracle::new(sec.oracle.clone()),
            aegis: Aegis::new(sec.aegis.clone()),
            ai: AiEngine::new(sec.ai.clone()),
            nexus: Nexus::new(
                sec.nexus.clone(),
                &config.kaya.host,
                config.kaya.port,
            ),
            veil: Veil::new(sec.veil.clone()),
            wasm: WasmRuntime::new(sec.wasm.clone()),
        })
    }

    /// Initialize all engines.
    pub async fn init(&mut self) -> Result<()> {
        // Initialize engines in parallel
        tokio::try_join!(
            async { self.sentinel.init().await.map_err(|e| anyhow::anyhow!(e)) },
            async { self.arbiter.init().await.map_err(|e| anyhow::anyhow!(e)) },
            async { self.oracle.init().await.map_err(|e| anyhow::anyhow!(e)) },
            async { self.aegis.init().await.map_err(|e| anyhow::anyhow!(e)) },
            async { self.ai.init().await.map_err(|e| anyhow::anyhow!(e)) },
            async { self.wasm.init().await.map_err(|e| anyhow::anyhow!(e)) },
        )?;

        tracing::info!(
            "Pentagon initialized: SENTINEL={}, ARBITER={}, ORACLE={}, AEGIS={}, AI={}, WASM={}",
            self.sentinel.is_ready(),
            self.arbiter.is_ready(),
            self.oracle.is_ready(),
            self.aegis.is_ready(),
            self.ai.is_ready(),
            self.wasm.is_ready(),
        );

        Ok(())
    }

    /// Inspect a request through all engines in parallel, then aggregate via NEXUS.
    pub async fn inspect(&self, ctx: &RequestContext) -> Result<FinalVerdict> {
        // Run all 5 security engines + WASM in parallel
        let (sentinel_r, arbiter_r, oracle_r, aegis_r, ai_r, wasm_r) = tokio::join!(
            self.sentinel.inspect(ctx),
            self.arbiter.inspect(ctx),
            self.oracle.inspect(ctx),
            self.aegis.inspect(ctx),
            self.ai.inspect(ctx),
            self.wasm.inspect(ctx),
        );

        // Collect decisions (log errors but continue with available results)
        let mut decisions: Vec<Decision> = Vec::with_capacity(6);

        for (name, result) in [
            ("SENTINEL", sentinel_r),
            ("ARBITER", arbiter_r),
            ("ORACLE", oracle_r),
            ("AEGIS", aegis_r),
            ("AI", ai_r),
            ("WASM", wasm_r),
        ] {
            match result {
                Ok(decision) => decisions.push(decision),
                Err(e) => tracing::error!("{} engine error: {}", name, e),
            }
        }

        // NEXUS aggregation
        let verdict = self.nexus.aggregate(ctx, &decisions);

        tracing::debug!(
            request_id = %ctx.request_id,
            action = ?verdict.action,
            score = verdict.score,
            "Pentagon verdict: {}",
            verdict.reason,
        );

        Ok(verdict)
    }

    /// Shut down all engines.
    pub async fn shutdown(&self) -> Result<()> {
        let _ = tokio::join!(
            self.sentinel.shutdown(),
            self.arbiter.shutdown(),
            self.oracle.shutdown(),
            self.aegis.shutdown(),
            self.ai.shutdown(),
            self.wasm.shutdown(),
        );
        tracing::info!("all Pentagon engines shut down");
        Ok(())
    }
}
