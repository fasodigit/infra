// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Common trait shared by all load-balancing algorithm implementations.

use crate::endpoint::Endpoint;
use std::sync::Arc;

// -- trait --

/// A load-balancing algorithm that picks one upstream endpoint per request.
///
/// Implementations must be `Send + Sync` because the gateway holds a single
/// shared instance across all worker threads.
///
/// # Parameters
/// - `endpoints`: The current pool of upstream backends (may include unhealthy ones).
/// - `hash_key`:  For consistent-hash variants (ring, Maglev), the bytes to hash
///   (typically the client IP or a session token).  Stateless algorithms ignore it.
///
/// Returns `None` when no healthy endpoint is available.
pub trait LoadBalancer: Send + Sync {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>>;

    /// Short human-readable name used in metrics labels and logs.
    fn name(&self) -> &'static str;
}
