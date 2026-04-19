// xds-server: gRPC server implementing xDS v3 APIs (port 18000).
//
// Implements ADS (Aggregated Discovery Service) for ARMAGEDDON.
// ARMAGEDDON connects via a single bidirectional gRPC stream and
// receives all resource types (CDS, EDS, RDS, LDS, SDS) over it.
//
// The server watches the ConfigStore for changes and pushes updates
// to all connected ARMAGEDDON instances without requiring SIGHUP.

pub mod canary;
pub mod config;
pub mod convert;
pub mod generated;
pub mod prometheus_client;
pub mod server;
pub mod services;
pub mod subscription;

pub use config::ServerConfig;
pub use server::XdsServer;
