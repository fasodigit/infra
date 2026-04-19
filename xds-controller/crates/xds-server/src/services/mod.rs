// xDS gRPC service implementations.
//
// These implement the tonic-generated server traits for each xDS service.
// All services delegate to the shared ADS logic since ARMAGEDDON uses ADS.

pub mod ads;
pub mod canary_grpc;
pub mod cds;
pub mod eds;
pub mod lds;
pub mod rds;
pub mod sds;

pub use ads::AdsService;
pub use canary_grpc::CanaryGrpcService;
pub use cds::CdsService;
pub use eds::EdsService;
pub use lds::LdsService;
pub use rds::RdsService;
pub use sds::SdsService;
