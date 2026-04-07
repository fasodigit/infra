//! armageddon-common: Shared types, errors, and traits for the ARMAGEDDON security gateway.
//!
//! This crate provides the foundational abstractions used across all Pentagon engines.

pub mod context;
pub mod decision;
pub mod engine;
pub mod error;
pub mod types;

pub use context::RequestContext;
pub use decision::{Action, Decision, Severity, Verdict};
pub use engine::SecurityEngine;
pub use error::ArmageddonError;
