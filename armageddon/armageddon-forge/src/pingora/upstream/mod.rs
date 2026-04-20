// SPDX-License-Identifier: AGPL-3.0-or-later
//! Upstream subsystem for the Pingora gateway.
//!
//! This module groups every concern that happens *after* filters have
//! finished with the request and the gateway needs to pick / reach a
//! backend.  The sub-modules below are **stubs** scaffolded in gate M0 and
//! progressively filled by gates M2 – M4.
//!
//! | Sub-module       | Gate     | Replaces (hyper path)          |
//! |------------------|----------|--------------------------------|
//! | `selector`       | M2 #103  | `src/upstream_pool.rs::PoolKey`|
//! | `mtls`           | M2       | `armageddon-mesh` SPIFFE peer  |
//! | `circuit_breaker`| M2       | `src/circuit_breaker.rs`       |
//! | `health`         | M2       | `src/health*.rs`               |
//! | `lb`             | M2       | `src/proxy.rs::RoundRobin*`    |
//! | `retry`          | M2       | `armageddon-retry`             |

pub mod circuit_breaker;
pub mod health;
pub mod lb;
pub mod mtls;
pub mod retry;
pub mod selector;
