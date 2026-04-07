//! CDC (Change Data Capture) listener for YugabyteDB.

use crate::{ChangeEvent, SyncError};

/// Trait for CDC event handlers.
pub trait CdcHandler: Send + Sync {
    /// Called when a change event is received from YugabyteDB.
    fn on_change(&self, event: ChangeEvent) -> Result<(), SyncError>;
}

/// CDC listener configuration and connection state.
pub struct CdcListener {
    url: String,
    running: std::sync::atomic::AtomicBool,
}

impl CdcListener {
    pub fn new(url: String) -> Self {
        Self {
            url,
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Start listening for CDC events.
    pub async fn start<H: CdcHandler>(&self, _handler: H) -> Result<(), SyncError> {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        tracing::info!(url = %self.url, "CDC listener started (stub)");
        // TODO: actual YugabyteDB CDC connection
        Ok(())
    }

    /// Stop the CDC listener.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}
