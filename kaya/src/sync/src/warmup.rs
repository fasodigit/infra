//! Warm-up: pre-load data from YugabyteDB into KAYA on startup.

use crate::SyncError;

/// Configuration for the warm-up process.
#[derive(Debug, Clone)]
pub struct WarmupConfig {
    /// Tables/collections to warm up.
    pub sources: Vec<String>,
    /// Max rows to load per source.
    pub max_rows_per_source: usize,
    /// Batch size for loading.
    pub batch_size: usize,
}

impl Default for WarmupConfig {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            max_rows_per_source: 100_000,
            batch_size: 1000,
        }
    }
}

/// Warm-up runner.
pub struct WarmupRunner {
    config: WarmupConfig,
}

impl WarmupRunner {
    pub fn new(config: WarmupConfig) -> Self {
        Self { config }
    }

    /// Run the warm-up process: load data from YugabyteDB into the store.
    pub async fn run(&self, _store: &kaya_store::Store) -> Result<usize, SyncError> {
        let total_loaded = 0usize;

        for source in &self.config.sources {
            tracing::info!(source = %source, "warming up from source (stub)");
            // TODO: actual DB query and store population
        }

        tracing::info!(total = total_loaded, "warm-up complete");
        Ok(total_loaded)
    }
}
