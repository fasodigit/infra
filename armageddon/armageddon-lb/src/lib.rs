// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Load balancing algorithms for ARMAGEDDON : RoundRobin, LeastConnections,
//! Random, WeightedRoundRobin, RingHash/Maglev, Power-of-Two-Choices.
//!
//! All algorithms implement the [`LoadBalancer`] trait and are `Send + Sync`
//! so they can be stored behind a shared reference in the proxy core.
//!
//! # Quick-start
//! ```rust,ignore
//! use armageddon_lb::{RoundRobin, LoadBalancer, Endpoint};
//! use std::sync::Arc;
//!
//! let eps: Vec<Arc<Endpoint>> = vec![
//!     Arc::new(Endpoint::new("a", "10.0.0.1:8080", 1)),
//!     Arc::new(Endpoint::new("b", "10.0.0.2:8080", 1)),
//! ];
//! let lb = RoundRobin::new();
//! let chosen = lb.select(&eps, None).unwrap();
//! println!("routing to {}", chosen.address);
//! ```

// -- modules --

pub mod algorithm;
pub mod endpoint;
pub mod least_conn;
pub mod maglev;
pub mod outlier;
pub mod p2c;
pub mod random;
pub mod ring_hash;
pub mod round_robin;
pub mod weighted;

// -- re-exports --

pub use algorithm::LoadBalancer;
pub use endpoint::Endpoint;
pub use least_conn::LeastConnections;
pub use maglev::Maglev;
pub use outlier::{FailureKind, OutlierDetector};
pub use p2c::PowerOfTwoChoices;
pub use random::Random;
pub use ring_hash::RingHash;
pub use round_robin::RoundRobin;
pub use weighted::WeightedRoundRobin;
