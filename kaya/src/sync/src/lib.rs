//! KAYA Sync: CDC with YugabyteDB, outbox pattern, vector clocks, warm-up.

pub mod cdc;
pub mod outbox;
pub mod vector_clock;
pub mod warmup;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("CDC connection failed: {0}")]
    ConnectionFailed(String),

    #[error("outbox error: {0}")]
    Outbox(String),

    #[error("warm-up failed: {0}")]
    WarmupFailed(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("conflict detected for key: {0}")]
    Conflict(String),

    #[error("sync error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub enabled: bool,
    pub yugabyte_url: String,
    pub outbox_poll_ms: u64,
    pub batch_size: usize,
    pub warmup_on_start: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            yugabyte_url: String::new(),
            outbox_poll_ms: 100,
            batch_size: 1000,
            warmup_on_start: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Change event
// ---------------------------------------------------------------------------

/// A change data capture event from YugabyteDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    /// Table or entity name.
    pub source: String,
    /// Type of change.
    pub kind: ChangeKind,
    /// The primary key.
    pub key: String,
    /// New value (None for deletes).
    pub value: Option<serde_json::Value>,
    /// Logical timestamp.
    pub timestamp: u64,
    /// Vector clock for conflict resolution.
    pub vclock: vector_clock::VectorClock,
}

/// Kind of data change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Insert,
    Update,
    Delete,
}

// ---------------------------------------------------------------------------
// Sync Manager
// ---------------------------------------------------------------------------

/// Manages the CDC pipeline and warm-up process.
pub struct SyncManager {
    config: SyncConfig,
}

impl SyncManager {
    pub fn new(config: SyncConfig) -> Self {
        Self { config }
    }

    /// Start the CDC listener (would connect to YugabyteDB in production).
    pub async fn start(&self) -> Result<(), SyncError> {
        if !self.config.enabled {
            tracing::info!("sync disabled, skipping CDC startup");
            return Ok(());
        }

        tracing::info!(
            url = %self.config.yugabyte_url,
            "starting CDC listener"
        );

        // TODO: implement actual CDC connection
        Ok(())
    }

    /// Apply a change event to the local store.
    pub fn apply_change(
        &self,
        event: &ChangeEvent,
        store: &kaya_store::Store,
    ) -> Result<(), SyncError> {
        match event.kind {
            ChangeKind::Insert | ChangeKind::Update => {
                let value = event
                    .value
                    .as_ref()
                    .ok_or_else(|| SyncError::Internal("missing value for upsert".into()))?;
                let serialized = serde_json::to_vec(value)
                    .map_err(|e| SyncError::Serialization(e.to_string()))?;
                store
                    .set(event.key.as_bytes(), &serialized, None)
                    .map_err(|e| SyncError::Internal(e.to_string()))?;
            }
            ChangeKind::Delete => {
                store.del(&[event.key.as_bytes()]);
            }
        }
        Ok(())
    }

    pub fn config(&self) -> &SyncConfig {
        &self.config
    }
}
