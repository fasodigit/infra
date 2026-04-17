// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Per-type-url subscription state for the ADS consumer.
//!
//! Tracks the last ACK'd version and nonce so the client can:
//! - Send correct ACK/NACK headers after each DiscoveryResponse
//! - Resume from the right point after reconnection
//! - Deduplicate responses (same version + nonce → skip callback)

use std::collections::{HashMap, HashSet};

/// Subscription state for a single xDS resource type.
///
/// One `Subscription` per type_url (CDS, EDS, LDS, RDS, SDS).
/// All fields are from the perspective of the *client* (ARMAGEDDON), i.e.
/// version_info and nonce are what the server last sent and the client ACK'd.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// xDS type URL, e.g. `type.googleapis.com/envoy.config.cluster.v3.Cluster`.
    pub type_url: String,

    /// Resource names to subscribe to.  Empty means wildcard (all resources).
    pub resource_names: Vec<String>,

    /// The `version_info` from the last successfully processed (ACK'd)
    /// DiscoveryResponse.  Empty string on first subscription.
    pub version_info: String,

    /// The server-generated nonce from the last ACK'd DiscoveryResponse.
    /// Empty string means no response has been ACK'd yet.
    pub nonce: String,

    /// Whether we have already sent the initial subscription request.
    pub subscribed: bool,
}

impl Subscription {
    /// Create a new subscription for a type URL, initially unsubscribed.
    pub fn new(type_url: impl Into<String>, resource_names: Vec<String>) -> Self {
        Self {
            type_url: type_url.into(),
            resource_names,
            version_info: String::new(),
            nonce: String::new(),
            subscribed: false,
        }
    }

    /// Record a successful ACK — advance version and nonce.
    pub fn record_ack(&mut self, version: impl Into<String>, nonce: impl Into<String>) {
        self.version_info = version.into();
        self.nonce = nonce.into();
    }

    /// Returns `true` if this (version, nonce) pair has already been ACK'd.
    /// Used to suppress duplicate callbacks when the server re-sends an
    /// identical response (e.g. on reconnect before the server detects loss).
    pub fn is_duplicate(&self, version: &str, nonce: &str) -> bool {
        // If both nonce and version match what we last ACK'd, skip the callback.
        // We still send an ACK so the server does not stall.
        !self.nonce.is_empty() && self.nonce == nonce && self.version_info == version
    }
}

/// Tracks all per-type subscriptions for one ADS client instance.
#[derive(Debug)]
pub struct SubscriptionMap {
    inner: HashMap<String, Subscription>,
}

impl SubscriptionMap {
    /// Create with default subscriptions for all five xDS resource types.
    pub fn new_all_types() -> Self {
        use crate::proto::type_urls;
        let mut map = HashMap::new();
        for &url in type_urls::ALL {
            map.insert(url.to_string(), Subscription::new(url, vec![]));
        }
        Self { inner: map }
    }

    /// Get a mutable reference to the subscription for `type_url`.
    pub fn get_mut(&mut self, type_url: &str) -> Option<&mut Subscription> {
        self.inner.get_mut(type_url)
    }

    /// Returns an iterator over all subscriptions.
    pub fn all(&self) -> impl Iterator<Item = &Subscription> {
        self.inner.values()
    }

    /// Returns an iterator over all subscriptions (mutable).
    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Subscription> {
        self.inner.values_mut()
    }

    /// Unique set of subscribed type URLs.
    pub fn type_urls(&self) -> HashSet<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
    }
}
