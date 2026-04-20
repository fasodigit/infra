// SPDX-License-Identifier: AGPL-3.0-or-later
//! Upstream subsystem for the Pingora gateway.
//!
//! This module groups every concern that happens *after* filters have
//! finished with the request and the gateway needs to pick / reach a
//! backend.
//!
//! | Sub-module       | Gate     | Replaces (hyper path)          | Status |
//! |------------------|----------|--------------------------------|--------|
//! | `selector`       | M2 #103  | `src/upstream_pool.rs::PoolKey`| Live   |
//! | `lb`             | M2 #103  | `armageddon-lb::round_robin`   | Live   |
//! | `mtls`           | M2       | `armageddon-mesh` SPIFFE peer  | Stub   |
//! | `circuit_breaker`| M2       | `src/circuit_breaker.rs`       | Stub   |
//! | `health`         | M2       | `src/health*.rs`               | Stub   |
//! | `retry`          | M2       | `armageddon-retry`             | Stub   |
//!
//! The **selector** and **lb** sub-modules ported in M2 first-wave (#103)
//! preserve the SPIFFE-aware `PoolKey` invariants (bug_006) that prevent
//! plaintext-for-mTLS downgrade and cross-identity impersonation.

pub mod circuit_breaker;
pub mod health;
pub mod lb;
pub mod mtls;
pub mod retry;
pub mod selector;

// -- M2 first-wave re-exports (#103) ----------------------------------------

pub use lb::{LbPolicy, PowerOfTwoChoices, RoundRobin, Weighted};
pub use selector::{ClusterResolver, ClusterState, PoolKey, ResolvedPeer};
