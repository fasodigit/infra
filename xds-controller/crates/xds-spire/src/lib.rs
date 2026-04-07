// xds-spire: SPIRE integration for automatic mTLS certificate distribution.
//
// Connects to the SPIRE Workload API to:
//   1. Fetch SVIDs (SPIFFE Verifiable Identity Documents) for services
//   2. Watch for certificate rotations
//   3. Push updated certificates to the ConfigStore (KAYA COLLECTION: certificates)
//   4. xds-server then distributes them to ARMAGEDDON via SDS

pub mod client;
pub mod error;
pub mod rotator;

pub use client::SpireClient;
pub use error::SpireError;
pub use rotator::CertificateRotator;
