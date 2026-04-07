// Conversion between internal domain models (xds-store) and protobuf types.
//
// The ConfigStore uses clean Rust types (ClusterEntry, EndpointEntry, etc.)
// while the gRPC layer uses prost-generated protobuf types. This module
// bridges the two.

use crate::generated::envoy::config::cluster::v3 as proto_cluster;
use crate::generated::envoy::config::endpoint::v3 as proto_endpoint;
use crate::generated::envoy::config::listener::v3 as proto_listener;
use crate::generated::envoy::config::route::v3 as proto_route;
use crate::generated::envoy::extensions::transport_sockets::tls::v3 as proto_secret;
use crate::subscription::type_urls;

use prost::Message;
use prost_types::Any;
use xds_store::model::*;

/// Encode a protobuf message as an Any.
fn encode_any<M: Message>(type_url: &str, msg: &M) -> Any {
    Any {
        type_url: type_url.to_string(),
        value: msg.encode_to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Cluster -> proto
// ---------------------------------------------------------------------------

/// Convert a ClusterEntry to a protobuf Cluster and wrap in Any.
pub fn cluster_to_any(cluster: &ClusterEntry) -> Any {
    let proto = proto_cluster::Cluster {
        name: cluster.name.clone(),
        r#type: match cluster.discovery_type {
            DiscoveryType::Static => proto_cluster::cluster::DiscoveryType::Static as i32,
            DiscoveryType::StrictDns => proto_cluster::cluster::DiscoveryType::StrictDns as i32,
            DiscoveryType::LogicalDns => proto_cluster::cluster::DiscoveryType::LogicalDns as i32,
            DiscoveryType::Eds => proto_cluster::cluster::DiscoveryType::Eds as i32,
        },
        connect_timeout: Some(prost_types::Duration {
            seconds: (cluster.connect_timeout_ms / 1000) as i64,
            nanos: ((cluster.connect_timeout_ms % 1000) * 1_000_000) as i32,
        }),
        lb_policy: match cluster.lb_policy {
            LbPolicy::RoundRobin => proto_cluster::cluster::LbPolicy::RoundRobin as i32,
            LbPolicy::LeastRequest => proto_cluster::cluster::LbPolicy::LeastRequest as i32,
            LbPolicy::RingHash => proto_cluster::cluster::LbPolicy::RingHash as i32,
            LbPolicy::Random => proto_cluster::cluster::LbPolicy::Random as i32,
            LbPolicy::Maglev => proto_cluster::cluster::LbPolicy::Maglev as i32,
        },
        eds_cluster_config: if cluster.discovery_type == DiscoveryType::Eds {
            Some(proto_cluster::EdsClusterConfig {
                service_name: cluster.name.clone(),
            })
        } else {
            None
        },
        health_checks: cluster
            .health_check
            .as_ref()
            .map(|hc| {
                vec![proto_cluster::HealthCheck {
                    timeout: Some(prost_types::Duration {
                        seconds: (hc.timeout_ms / 1000) as i64,
                        nanos: ((hc.timeout_ms % 1000) * 1_000_000) as i32,
                    }),
                    interval: Some(prost_types::Duration {
                        seconds: (hc.interval_ms / 1000) as i64,
                        nanos: ((hc.interval_ms % 1000) * 1_000_000) as i32,
                    }),
                    unhealthy_threshold: Some(hc.unhealthy_threshold),
                    healthy_threshold: Some(hc.healthy_threshold),
                    health_checker: hc.path.as_ref().map(|p| {
                        proto_cluster::health_check::HealthChecker::HttpHealthCheck(
                            proto_cluster::HttpHealthCheck {
                                host: String::new(),
                                path: p.clone(),
                                expected_statuses: 200,
                            },
                        )
                    }),
                }]
            })
            .unwrap_or_default(),
        circuit_breakers: cluster.circuit_breaker.as_ref().map(|cb| {
            proto_cluster::CircuitBreakers {
                thresholds: vec![proto_cluster::circuit_breakers::Thresholds {
                    priority: proto_cluster::RoutingPriority::Default as i32,
                    max_connections: Some(cb.max_connections),
                    max_pending_requests: Some(cb.max_pending_requests),
                    max_requests: Some(cb.max_requests),
                    max_retries: Some(cb.max_retries),
                }],
            }
        }),
        transport_socket_tls: cluster.spiffe_id.as_ref().map(|id| {
            proto_cluster::UpstreamTlsContext {
                sni: String::new(),
                spiffe_id: id.clone(),
            }
        }),
        metadata: cluster.metadata.clone(),
    };

    encode_any(type_urls::CLUSTER, &proto)
}

// ---------------------------------------------------------------------------
// Endpoints -> proto
// ---------------------------------------------------------------------------

/// Convert endpoints for a cluster to a ClusterLoadAssignment Any.
pub fn endpoints_to_any(cluster_name: &str, endpoints: &[EndpointEntry]) -> Any {
    let lb_endpoints: Vec<proto_endpoint::LbEndpoint> = endpoints
        .iter()
        .map(|ep| proto_endpoint::LbEndpoint {
            health_status: match ep.health_status {
                HealthStatus::Unknown => proto_endpoint::HealthStatus::Unknown as i32,
                HealthStatus::Healthy => proto_endpoint::HealthStatus::Healthy as i32,
                HealthStatus::Unhealthy => proto_endpoint::HealthStatus::Unhealthy as i32,
                HealthStatus::Draining => proto_endpoint::HealthStatus::Draining as i32,
                HealthStatus::Timeout => proto_endpoint::HealthStatus::Timeout as i32,
                HealthStatus::Degraded => proto_endpoint::HealthStatus::Degraded as i32,
            },
            endpoint: Some(proto_endpoint::Endpoint {
                address: Some(proto_endpoint::Address {
                    socket_address: Some(proto_endpoint::SocketAddress {
                        address: ep.address.clone(),
                        port_value: ep.port as u32,
                        protocol: "TCP".to_string(),
                    }),
                }),
                health_check_config: None,
            }),
            load_balancing_weight: Some(ep.weight),
            metadata: ep.metadata.clone(),
        })
        .collect();

    let proto = proto_endpoint::ClusterLoadAssignment {
        cluster_name: cluster_name.to_string(),
        endpoints: vec![proto_endpoint::LocalityLbEndpoints {
            locality: None,
            lb_endpoints,
            load_balancing_weight: None,
            priority: 0,
        }],
        policy: None,
    };

    encode_any(type_urls::ENDPOINT, &proto)
}

// ---------------------------------------------------------------------------
// Routes -> proto
// ---------------------------------------------------------------------------

/// Convert a RouteEntry to a protobuf RouteConfiguration Any.
pub fn route_to_any(route: &RouteEntry) -> Any {
    let virtual_hosts: Vec<proto_route::VirtualHost> = route
        .virtual_hosts
        .iter()
        .map(|vh| proto_route::VirtualHost {
            name: vh.name.clone(),
            domains: vh.domains.clone(),
            routes: vh
                .routes
                .iter()
                .map(|r| {
                    let path_specifier = match &r.path_match {
                        PathMatch::Prefix(p) => {
                            Some(proto_route::route_match::PathSpecifier::Prefix(p.clone()))
                        }
                        PathMatch::Exact(p) => {
                            Some(proto_route::route_match::PathSpecifier::Path(p.clone()))
                        }
                        PathMatch::Regex(p) => {
                            Some(proto_route::route_match::PathSpecifier::SafeRegex(p.clone()))
                        }
                    };

                    let headers: Vec<proto_route::HeaderMatcher> = r
                        .header_matchers
                        .iter()
                        .map(|hm| proto_route::HeaderMatcher {
                            name: hm.name.clone(),
                            header_match_specifier: Some(match hm.match_type {
                                HeaderMatchType::Exact => {
                                    proto_route::header_matcher::HeaderMatchSpecifier::ExactMatch(
                                        hm.value.clone(),
                                    )
                                }
                                HeaderMatchType::Prefix => {
                                    proto_route::header_matcher::HeaderMatchSpecifier::PrefixMatch(
                                        hm.value.clone(),
                                    )
                                }
                                HeaderMatchType::Suffix => {
                                    proto_route::header_matcher::HeaderMatchSpecifier::SuffixMatch(
                                        hm.value.clone(),
                                    )
                                }
                                HeaderMatchType::Contains => {
                                    proto_route::header_matcher::HeaderMatchSpecifier::ContainsMatch(
                                        hm.value.clone(),
                                    )
                                }
                                HeaderMatchType::Regex => {
                                    proto_route::header_matcher::HeaderMatchSpecifier::SafeRegexMatch(
                                        hm.value.clone(),
                                    )
                                }
                            }),
                            invert_match: hm.invert,
                        })
                        .collect();

                    let cluster_specifier = if let Some(wc) = &r.weighted_clusters {
                        Some(proto_route::route_action::ClusterSpecifier::WeightedClusters(
                            proto_route::WeightedClusters {
                                clusters: wc
                                    .iter()
                                    .map(|c| proto_route::weighted_clusters::ClusterWeight {
                                        name: c.name.clone(),
                                        weight: Some(c.weight),
                                        metadata: Default::default(),
                                    })
                                    .collect(),
                            },
                        ))
                    } else {
                        Some(proto_route::route_action::ClusterSpecifier::Cluster(
                            r.cluster.clone(),
                        ))
                    };

                    let timeout = r.timeout_ms.map(|ms| prost_types::Duration {
                        seconds: (ms / 1000) as i64,
                        nanos: ((ms % 1000) * 1_000_000) as i32,
                    });

                    let retry_policy = r.retry_policy.as_ref().map(|rp| proto_route::RetryPolicy {
                        retry_on: rp.retry_on.clone(),
                        num_retries: Some(rp.num_retries),
                        per_try_timeout: rp.per_try_timeout_ms.map(|ms| prost_types::Duration {
                            seconds: (ms / 1000) as i64,
                            nanos: ((ms % 1000) * 1_000_000) as i32,
                        }),
                    });

                    proto_route::Route {
                        r#match: Some(proto_route::RouteMatch {
                            path_specifier,
                            headers,
                            query_parameters: vec![],
                        }),
                        action: Some(proto_route::route::Action::Route(proto_route::RouteAction {
                            cluster_specifier,
                            timeout,
                            retry_policy,
                            request_headers_to_add: vec![],
                            prefix_rewrite: r.prefix_rewrite.clone().unwrap_or_default(),
                            host_rewrite_literal: String::new(),
                            hash_policy: vec![],
                        })),
                        metadata: Default::default(),
                        name: r.name.clone().unwrap_or_default(),
                    }
                })
                .collect(),
            request_headers_to_add: vec![],
            response_headers_to_add: vec![],
            rate_limits: vec![],
        })
        .collect();

    let proto = proto_route::RouteConfiguration {
        name: route.name.clone(),
        virtual_hosts,
    };

    encode_any(type_urls::ROUTE, &proto)
}

// ---------------------------------------------------------------------------
// Listener -> proto
// ---------------------------------------------------------------------------

/// Convert a ListenerEntry to a protobuf Listener Any.
pub fn listener_to_any(listener: &ListenerEntry) -> Any {
    let filter_chains: Vec<proto_listener::FilterChain> = listener
        .filter_chains
        .iter()
        .map(|fc| proto_listener::FilterChain {
            filter_chain_match: if fc.server_names.is_empty() {
                None
            } else {
                Some(proto_listener::FilterChainMatch {
                    destination_port: None,
                    server_names: fc.server_names.clone(),
                    application_protocols: vec![],
                    source_prefix_ranges: vec![],
                    source_ports: vec![],
                })
            },
            filters: vec![],
            transport_socket_tls: None,
            name: fc.name.clone().unwrap_or_default(),
        })
        .collect();

    let proto = proto_listener::Listener {
        name: listener.name.clone(),
        address: Some(proto_listener::Address {
            address: Some(proto_listener::address::Address::SocketAddress(
                proto_listener::SocketAddress {
                    address: listener.address.clone(),
                    port_value: listener.port as u32,
                    protocol: "TCP".to_string(),
                },
            )),
        }),
        filter_chains,
        default_filter_chain: None,
        per_connection_buffer_limit_bytes: None,
        metadata: Default::default(),
        listener_filters: vec![],
    };

    encode_any(type_urls::LISTENER, &proto)
}

// ---------------------------------------------------------------------------
// Certificate -> proto
// ---------------------------------------------------------------------------

/// Convert a CertificateEntry to a protobuf Secret Any.
pub fn certificate_to_any(cert: &CertificateEntry) -> Any {
    let proto = proto_secret::Secret {
        name: cert.spiffe_id.clone(),
        r#type: Some(proto_secret::secret::Type::TlsCertificate(
            proto_secret::TlsCertificate {
                certificate_chain: Some(proto_secret::DataSource {
                    specifier: Some(proto_secret::data_source::Specifier::InlineString(
                        cert.certificate_chain.clone(),
                    )),
                }),
                private_key: Some(proto_secret::DataSource {
                    specifier: Some(proto_secret::data_source::Specifier::InlineString(
                        cert.private_key.clone(),
                    )),
                }),
                ocsp_staple: None,
            },
        )),
    };

    encode_any(type_urls::SECRET, &proto)
}
