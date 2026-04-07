// Certificate rotator: watches SPIRE for certificate renewals
// and pushes updated certificates to the ConfigStore.

use crate::client::{SpireClient, Svid};
use crate::SpireError;
use chrono::Utc;
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use xds_store::{CertificateEntry, ConfigStore};

/// Watches SPIRE for certificate rotations and updates the store.
pub struct CertificateRotator {
    /// SPIRE client for fetching SVIDs.
    client: Arc<SpireClient>,

    /// Configuration store to update (KAYA COLLECTION: certificates).
    store: ConfigStore,

    /// How often to check for rotation (well before expiry).
    poll_interval: Duration,

    /// Rotate certificates when this fraction of lifetime has elapsed.
    /// E.g., 0.5 means rotate at 50% of certificate lifetime.
    rotation_threshold: f64,
}

impl CertificateRotator {
    pub fn new(
        client: Arc<SpireClient>,
        store: ConfigStore,
        poll_interval: Duration,
    ) -> Self {
        Self {
            client,
            store,
            poll_interval,
            rotation_threshold: 0.5,
        }
    }

    /// Set the rotation threshold (fraction of certificate lifetime).
    pub fn with_rotation_threshold(mut self, threshold: f64) -> Self {
        self.rotation_threshold = threshold.clamp(0.1, 0.9);
        self
    }

    /// Run the certificate rotation loop.
    pub async fn run(self) -> Result<(), SpireError> {
        info!(
            interval_ms = self.poll_interval.as_millis() as u64,
            threshold = self.rotation_threshold,
            "starting certificate rotator"
        );

        let mut interval = time::interval(self.poll_interval);

        loop {
            interval.tick().await;

            match self.client.fetch_svid().await {
                Ok(svids) => {
                    for svid in svids {
                        self.process_svid(svid);
                    }
                }
                Err(SpireError::AgentUnavailable { .. }) => {
                    warn!("SPIRE agent not available, will retry");
                }
                Err(e) => {
                    error!(error = %e, "failed to fetch SVIDs from SPIRE");
                }
            }
        }
    }

    /// Process a single SVID: update the store if the certificate is new or rotated.
    fn process_svid(&self, svid: Svid) {
        let now = Utc::now();

        // Check if we already have this certificate and it's still valid
        if let Some(existing) = self.store.get_certificate(&svid.spiffe_id) {
            if existing.expires_at == svid.expires_at {
                // Same certificate, no rotation needed
                return;
            }
        }

        let entry = CertificateEntry {
            spiffe_id: svid.spiffe_id.clone(),
            certificate_chain: svid.certificate_chain,
            private_key: svid.private_key,
            trusted_ca: Some(svid.trust_bundle),
            expires_at: svid.expires_at,
            rotated_at: now,
            updated_at: now,
        };

        match self.store.set_certificate(entry) {
            Ok(version) => {
                info!(
                    spiffe_id = %svid.spiffe_id,
                    version = %version,
                    expires_at = %svid.expires_at,
                    "certificate rotated in store"
                );
            }
            Err(e) => {
                error!(
                    spiffe_id = %svid.spiffe_id,
                    error = %e,
                    "failed to update certificate in store"
                );
            }
        }
    }
}
