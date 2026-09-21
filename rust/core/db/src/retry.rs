//! Retry for retryable database failures.
//!
//! WHAT THIS IS FOR. Neon suspends idle databases and the pool can hit a cold connect; a transient
//! `DatabaseUnavailable` or `Timeout` is worth trying again rather than surfacing as a 500. That behavior existed in
//! the TypeScript gateway and did not survive the port.
//!
//! WHAT IT IS NOT. This is not a blanket "try every query three times". Retrying is only safe for operations that can
//! be repeated without changing the result - a read, or a write already guarded by a claim/receipt. Retrying an
//! unguarded write turns one intended insert into two. So the policy is explicit at the call site and the decision
//! belongs to whoever knows whether the statement is idempotent.
//!
//! Only failures the taxonomy marks `retryable` (DatabaseUnavailable, Timeout) are retried. A constraint violation or
//! a schema mismatch is retried zero times, because waiting does not fix either.

use std::future::Future;
use std::time::Duration;

use crate::error::{DbFailure, DbResult};

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(150),
            max_delay: Duration::from_millis(2_000),
        }
    }
}

impl RetryPolicy {
    /// From the environment, so a cold database can be given more room without a code change: `FORGE_DB_RETRY_ATTEMPTS`,
    /// `FORGE_DB_RETRY_BASE_MS`.
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            attempts: positive_u32("FORGE_DB_RETRY_ATTEMPTS", default.attempts),
            base_delay: Duration::from_millis(positive_u64(
                "FORGE_DB_RETRY_BASE_MS",
                default.base_delay.as_millis() as u64,
            )),
            max_delay: default.max_delay,
        }
    }
}

/// Run `operation`, retrying only retryable failures, and announce every failure once.
pub async fn retry<T, F, Fut>(policy: RetryPolicy, mut operation: F) -> DbResult<T>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = DbResult<T>>,
{
    let attempts = policy.attempts.max(1);
    let mut attempt = 0;
    loop {
        attempt += 1;
        match operation(attempt).await {
            Ok(value) => return Ok(value),
            Err(failure) => {
                crate::capture::notify(&failure);
                if !failure.retryable || attempt >= attempts {
                    return Err(failure);
                }
                tokio::time::sleep(backoff(policy, attempt)).await;
            }
        }
    }
}

/// Exponential, capped, with jitter. Jitter matters: without it, every caller that failed at the same moment retries at
/// the same moment, which is how a cold database gets stampeded by the pool that is waiting for it.
fn backoff(policy: RetryPolicy, attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let scaled = policy
        .base_delay
        .saturating_mul(1u32 << exponent)
        .min(policy.max_delay);
    // Deterministic-ish jitter from the clock, so tests do not need a random source and behavior stays reproducible
    // at the millisecond scale that matters here.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos() as u64)
        .unwrap_or(0);
    let jitter = nanos % (scaled.as_millis().max(1) as u64);
    scaled.saturating_add(Duration::from_millis(jitter / 2))
}

fn positive_u32(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn positive_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DbFailureKind;

    fn failure(retryable: bool) -> DbFailure {
        DbFailure {
            kind: if retryable {
                DbFailureKind::DatabaseUnavailable
            } else {
                DbFailureKind::Constraint
            },
            operation: "test.select",
            incident_id: uuid::Uuid::nil(),
            code: None,
            detail: None,
            retryable,
        }
    }

    fn fast() -> RetryPolicy {
        RetryPolicy {
            attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        }
    }

    #[tokio::test]
    async fn a_retryable_failure_is_tried_again_and_can_succeed() {
        let mut calls = 0;
        let result = retry(fast(), |_| {
            calls += 1;
            let attempt = calls;
            async move {
                if attempt < 3 {
                    Err(failure(true))
                } else {
                    Ok("ok")
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn a_non_retryable_failure_is_not_retried() {
        let mut calls = 0;
        let result: DbResult<()> = retry(fast(), |_| {
            calls += 1;
            async { Err(failure(false)) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1, "a constraint violation is not fixed by waiting");
    }

    #[tokio::test]
    async fn attempts_are_bounded() {
        let mut calls = 0;
        let result: DbResult<()> = retry(fast(), |_| {
            calls += 1;
            async { Err(failure(true)) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 3);
    }
}
