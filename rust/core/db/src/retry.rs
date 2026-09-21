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

use crate::error::DbResult;

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

/// The policy from the environment, parsed once. These are deployment settings, not per-request values, and reading the
/// environment on every query would be silly.
pub fn policy() -> RetryPolicy {
    static POLICY: std::sync::OnceLock<RetryPolicy> = std::sync::OnceLock::new();
    *POLICY.get_or_init(RetryPolicy::from_env)
}

/// Announce a failed attempt. Public so the `retrying_read!` macro can reach it from other crates.
pub fn notice_failure(failure: &crate::error::DbFailure) {
    crate::capture::notify(failure);
}

/// Wait before the next attempt. Public for the same reason.
pub async fn sleep_before_retry(policy: RetryPolicy, attempt: u32) {
    tokio::time::sleep(backoff(policy, attempt)).await;
}

/// Retry a read.
///
/// WHY A MACRO. The natural shape would be a function taking the operation as a closure, but the operations that need
/// this borrow the DAO mutably, and a closure returning a future that borrows its captured state cannot be called
/// again on the next iteration - the borrow checker has no way to know the previous future is finished. Writing the
/// operation inside the loop body sidesteps that entirely, and a macro is the way to write it inside the loop body at
/// every call site without copying the loop.
///
/// Use this ONLY for operations that can be repeated without changing the result: reads, or writes already guarded by
/// a claim or receipt. An unguarded write retried once is two rows.
#[macro_export]
macro_rules! retrying_read {
    ($operation:expr) => {{
        let policy = $crate::retry::policy();
        let attempts = policy.attempts.max(1);
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match $operation.await {
                Ok(value) => break Ok(value),
                Err(failure) => {
                    $crate::retry::notice_failure(&failure);
                    if !failure.retryable || attempt >= attempts {
                        break Err(failure);
                    }
                    $crate::retry::sleep_before_retry(policy, attempt).await;
                }
            }
        }
    }};
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
    use crate::error::{DbFailure, DbFailureKind};

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

    #[tokio::test]
    async fn the_macro_retries_an_operation_that_borrows_self_mutably() {
        // This mirrors the real call sites: an `impl` method retried through the macro, borrowing `&mut self`. That

        // shape is the whole reason the macro exists - a closure returning a future that borrows its captured state
        // cannot be called a second time, and this test fails to compile if anyone rewrites it back into a closure.
        struct Flaky {
            calls: u32,
        }
        impl Flaky {
            async fn read(&mut self) -> DbResult<&'static str> {
                self.calls += 1;
                if self.calls < 2 {
                    Err(failure(true))
                } else {
                    Ok("ok")
                }
            }
        }

        async fn read_it(flaky: &mut Flaky) -> DbResult<&'static str> {
            crate::retrying_read!(flaky.read())
        }

        let mut flaky = Flaky { calls: 0 };
        assert_eq!(read_it(&mut flaky).await.unwrap(), "ok");
        assert_eq!(flaky.calls, 2, "the first attempt failed and was repeated");
    }

    #[tokio::test]
    async fn the_macro_does_not_repeat_a_non_retryable_failure() {
        struct Fixed;
        impl Fixed {
            async fn read(&mut self, calls: &mut u32) -> DbResult<()> {
                *calls += 1;
                Err(failure(false))
            }
        }

        async fn read_it(fixed: &mut Fixed, calls: &mut u32) -> DbResult<()> {
            crate::retrying_read!(fixed.read(calls))
        }

        let (mut fixed, mut calls) = (Fixed, 0);
        assert!(read_it(&mut fixed, &mut calls).await.is_err());
        assert_eq!(calls, 1);
    }
}
