// Auto-generated protobuf/tonic modules.
// Compiled from proto/ by build.rs via tonic-build and prost.
//
// Module structure mirrors the protobuf package hierarchy:
//   envoy.service.discovery.v3 -> envoy::service::discovery::v3
//   envoy.config.cluster.v3    -> envoy::config::cluster::v3
//   google.rpc                 -> google::rpc

pub mod google {
    pub mod rpc {
        include!("google.rpc.rs");
    }
}

pub mod envoy {
    pub mod config {
        pub mod cluster {
            pub mod v3 {
                include!("envoy.config.cluster.v3.rs");
            }
        }
        pub mod endpoint {
            pub mod v3 {
                include!("envoy.config.endpoint.v3.rs");
            }
        }
        pub mod route {
            pub mod v3 {
                include!("envoy.config.route.v3.rs");
            }
        }
        pub mod listener {
            pub mod v3 {
                include!("envoy.config.listener.v3.rs");
            }
        }
    }

    pub mod extensions {
        pub mod transport_sockets {
            pub mod tls {
                pub mod v3 {
                    include!("envoy.extensions.transport_sockets.tls.v3.rs");
                }
            }
        }
    }

    pub mod service {
        pub mod discovery {
            pub mod v3 {
                include!("envoy.service.discovery.v3.rs");
            }
        }
        pub mod cluster {
            pub mod v3 {
                include!("envoy.service.cluster.v3.rs");
            }
        }
        pub mod endpoint {
            pub mod v3 {
                include!("envoy.service.endpoint.v3.rs");
            }
        }
        pub mod route {
            pub mod v3 {
                include!("envoy.service.route.v3.rs");
            }
        }
        pub mod listener {
            pub mod v3 {
                include!("envoy.service.listener.v3.rs");
            }
        }
        pub mod secret {
            pub mod v3 {
                include!("envoy.service.secret.v3.rs");
            }
        }
    }
}
