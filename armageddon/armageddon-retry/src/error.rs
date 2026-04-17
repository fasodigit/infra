// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Error types for the retry / timeout / budget subsystem.

use thiserror::Error;
use std::time::Duration;

/// All errors that can be produced by the retry engine.
#[derive(Error, Debug)]
pub enum RetryError {
    /// All retry attempts were consumed without a successful response.
    #[error("retry budget exhausted after {attempts} attempt(s)")]
    Exhausted { attempts: u32 },

    /// The overall deadline elapsed before a response was obtained.
    #[error("overall timeout ({timeout:?}) exceeded after {attempts} attempt(s)")]
    Timeout { timeout: Duration, attempts: u32 },

    /// The retry budget was depleted; the request was not retried.
    #[error("retry budget depleted — retry skipped to prevent cascade failure")]
    BudgetDepleted,

    /// A per-try timeout occurred on the final attempt with no retry remaining.
    #[error("per-try timeout ({timeout:?}) on final attempt")]
    PerTryTimeout { timeout: Duration },

    /// The underlying call returned an error that is not retryable.
    #[error("non-retryable upstream error: {0}")]
    NonRetryable(String),
}
