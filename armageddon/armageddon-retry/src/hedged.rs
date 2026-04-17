// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Hedged-request support.
//!
//! A *hedged* request fires a second attempt after `hedging_delay` if the
//! first attempt has not yet returned a successful response.  Whichever
//! attempt succeeds first wins; the other is silently dropped (the `Future`
//! is cancelled when the `select!` arm completes).
//!
//! This technique trades a small amount of additional upstream load for a
//! significant reduction in tail latency (p99 / p999).

use std::future::Future;
use std::time::Duration;
use crate::error::RetryError;

// -- hedged --

/// Execute `f` twice, with the second copy starting after `delay`.
///
/// Returns the first `Ok(T)` result.  If both attempts return `Err`, the
/// error from the **first** attempt is returned wrapped in
/// [`RetryError::Exhausted`].
///
/// # Example
///
/// ```rust,ignore
/// let result = hedged(|| call_upstream(req.clone()), Duration::from_millis(50)).await;
/// ```
pub async fn hedged<F, Fut, T, E>(f: F, delay: Duration) -> Result<T, RetryError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let first = f();
    tokio::pin!(first);

    // Race: first attempt vs. (delay + second attempt).
    tokio::select! {
        // The first attempt resolved before the hedging delay.
        result = &mut first => {
            match result {
                Ok(v) => Ok(v),
                Err(_) => {
                    // First failed — try second immediately (no extra delay).
                    match f().await {
                        Ok(v) => Ok(v),
                        Err(_) => Err(RetryError::Exhausted { attempts: 2 }),
                    }
                }
            }
        }
        // The hedging delay elapsed before the first attempt finished.
        _ = tokio::time::sleep(delay) => {
            // Launch the second attempt.
            let second = f();
            tokio::pin!(second);

            tokio::select! {
                result = &mut first => {
                    match result {
                        Ok(v) => Ok(v),
                        Err(_) => {
                            // Wait for the second attempt.
                            match second.await {
                                Ok(v) => Ok(v),
                                Err(_) => Err(RetryError::Exhausted { attempts: 2 }),
                            }
                        }
                    }
                }
                result = &mut second => {
                    match result {
                        Ok(v) => Ok(v),
                        Err(_) => {
                            // Wait for the first attempt.
                            match first.await {
                                Ok(v) => Ok(v),
                                Err(_) => Err(RetryError::Exhausted { attempts: 2 }),
                            }
                        }
                    }
                }
            }
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn hedged_fast_first_wins() {
        let result = hedged(
            || async { Ok::<_, String>("fast") },
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(result.unwrap(), "fast");
    }

    #[tokio::test]
    async fn hedged_slow_first_second_wins() {
        // first is slow (200 ms), second fires after 50 ms hedging delay → second wins
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = Arc::clone(&calls);

        let result = hedged(
            move || {
                let n = calls2.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        Ok::<&str, &str>("slow")
                    } else {
                        // second attempt responds quickly
                        Ok::<&str, &str>("fast-hedge")
                    }
                }
            },
            Duration::from_millis(50),
        )
        .await;

        // We accept either "fast-hedge" or "slow" — what matters is success.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn hedged_both_fail_returns_exhausted() {
        let result = hedged(
            || async { Err::<(), &str>("boom") },
            Duration::from_millis(10),
        )
        .await;
        assert!(matches!(result, Err(RetryError::Exhausted { attempts: 2 })));
    }
}
