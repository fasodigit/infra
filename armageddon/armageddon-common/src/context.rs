//! Request context flowing through the Pentagon pipeline.

use crate::types::{ConnectionInfo, HttpRequest, Protocol, RequestId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full request context passed to every security engine.
///
/// Created once at ingress by FORGE, enriched by each engine, and finally
/// consumed by NEXUS for aggregation and scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// Unique request identifier (UUIDv4).
    pub request_id: RequestId,

    /// Timestamp when the request was received.
    pub timestamp: DateTime<Utc>,

    /// The HTTP request being inspected.
    pub request: HttpRequest,

    /// Connection-level metadata (IP, TLS, JA3).
    pub connection: ConnectionInfo,

    /// Detected protocol type.
    pub protocol: Protocol,

    /// JWT claims if authentication succeeded (populated by FORGE jwt_authn).
    pub jwt_claims: Option<HashMap<String, serde_json::Value>>,

    /// Matched route name (populated by FORGE after routing).
    pub matched_route: Option<String>,

    /// Target cluster name (populated by FORGE after routing).
    pub target_cluster: Option<String>,

    /// GeoIP information (populated by SENTINEL).
    pub geo: Option<GeoInfo>,

    /// Arbitrary metadata attached by engines.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl RequestContext {
    /// Create a new context for an incoming request.
    pub fn new(request: HttpRequest, connection: ConnectionInfo, protocol: Protocol) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request,
            connection,
            protocol,
            jwt_claims: None,
            matched_route: None,
            target_cluster: None,
            geo: None,
            metadata: HashMap::new(),
        }
    }
}

/// GeoIP location data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    pub country_code: String,
    pub country_name: String,
    pub city: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub asn: Option<u32>,
    pub as_org: Option<String>,
}
