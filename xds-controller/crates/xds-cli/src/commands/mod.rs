// CLI command handlers.
//
// Each submodule handles one resource type.
// In the future, these will use a gRPC management API
// to communicate with the xDS Controller.

pub mod cluster;
pub mod endpoint;
pub mod listener;
pub mod route;
pub mod snapshot;
pub mod status;
