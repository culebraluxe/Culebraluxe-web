//! Deterministic driver faults at the database pool boundary.
//!
//! WHY THIS EXISTS. The pool's timeout contract is not a function of a working database — it is a function of what
//! the pool does when a checkout does not arrive: `PgPool::acquire` returns `sqlx::Error::PoolTimedOut` once
//! `acquire_timeout` expires, and a server-side `statement_timeout` arrives as SQLSTATE `57014` (`query_canceled`).
//! A test that needs a slow server to observe either is not deterministic and cannot run without a live endpoint. So
//! this harness is the smallest honest seam: it hands the production classifiers the exact driver-shaped errors the
//! pool produces, and never decides anything itself.
//!
//! WHAT IT IS NOT. It does not classify, retry or adjudicate. A [`DbPoolFaultHarness::acquire`] runs the production
//! `DbFailure::from_sqlx` on a driver error and returns that production result; the retry policy under test is
//! production's own (`db::retry`). If this module ever grew its own kind table, a test could pass because the harness,
//! not the pool, classified the fault.
//!
//! Level: L1 Component (a scripted collaborator) — reused by L4 Adversarial tests that drive many callers at once.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::error::Error as StdError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use db::{DbFailure, DbResult};
use sqlx::error::{DatabaseError, ErrorKind};

/// A driver error carrying a SQLSTATE, shaped exactly like the one sqlx decodes from Postgres.
///
/// sqlx exposes `sqlx::error::DatabaseError` so a caller can hand the driver's own error type to a classifier; this
/// fake is that type for a test, and it exists so SQLSTATE-driven behaviour can be exercised with no socket.
#[derive(Debug)]
struct SqlStateFault {
    code: &'static str,
    message: &'static str,
}

impl std::fmt::Display for SqlStateFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl StdError for SqlStateFault {}

impl DatabaseError for SqlStateFault {
    fn message(&self) -> &str {
        self.message
    }

    fn code(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.code))
    }

    fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

/// One outcome a pool may hand a caller at checkout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolFault {
    /// A connection is available: the healthy path.
    Ready,
    /// `PgPool::acquire` waited past `acquire_timeout`: the exact `sqlx::Error::PoolTimedOut`.
    PoolTimedOut,
    /// A statement was cancelled by the server's `statement_timeout`: SQLSTATE `57014`.
    StatementTimeout,
    /// A lock wait was cancelled: SQLSTATE `55P03`.
    LockTimeout,
    /// `idle_in_transaction_session_timeout` fired: SQLSTATE `25P03`. The name says "timeout", but it TERMINATES THE
    /// SESSION rather than cancelling a statement, so it is a connection failure (`DatabaseUnavailable`), not a
    /// `Timeout` — the trap the classifier explicitly refuses (`db/src/error.rs:161-170`).
    IdleInTransactionTimeout,
    /// The server has no free connection slots: SQLSTATE `53300`. A pool-boundary connection failure, not a `Timeout`.
    ConnectionExhausted,
    /// A unique-key violation: SQLSTATE `23505` — a work failure, not a timeout.
    ConstraintViolation,
    /// A missing relation: SQLSTATE `42P01` — a work failure, not a timeout.
    SchemaMismatch,
}

impl PoolFault {
    /// The driver error this fault produces, or `None` when the pool has a connection to give.
    pub fn driver_error(self) -> Option<sqlx::Error> {
        match self {
            Self::Ready => None,
            // The value sqlx itself builds when the acquire deadline passes; the pool never invents this.
            Self::PoolTimedOut => Some(sqlx::Error::PoolTimedOut),
            Self::StatementTimeout => Some(sqlstate(
                "57014",
                "canceling statement due to statement timeout",
            )),
            Self::LockTimeout => Some(sqlstate("55P03", "canceling statement due to lock timeout")),
            Self::IdleInTransactionTimeout => Some(sqlstate(
                "25P03",
                "terminating connection due to idle-in-transaction timeout",
            )),
            Self::ConnectionExhausted => Some(sqlstate("53300", "sorry, too many clients already")),
            Self::ConstraintViolation => Some(sqlstate(
                "23505",
                "duplicate key value violates unique constraint \"pool_fault_key\"",
            )),
            Self::SchemaMismatch => Some(sqlstate(
                "42P01",
                "relation \"public.absent\" does not exist",
            )),
        }
    }
}

fn sqlstate(code: &'static str, message: &'static str) -> sqlx::Error {
    sqlx::Error::Database(Box::new(SqlStateFault { code, message }))
}

/// A scripted pool boundary: it hands out the driver's own errors, in order, and counts what it gave.
///
/// The script is deterministic and replayable; past its end every call takes the fallback (a connection unless the
/// caller says otherwise). It performs no I/O and never touches a database — the fault is a value, and the production
/// classifier is what turns it into a `DbFailure`.
#[derive(Debug)]
pub struct DbPoolFaultHarness {
    script: Mutex<VecDeque<PoolFault>>,
    fallback: PoolFault,
    attempts: AtomicU64,
    successes: AtomicU64,
}

impl DbPoolFaultHarness {
    /// A pool that follows `script` and is healthy afterwards.
    pub fn scripted(script: Vec<PoolFault>) -> Self {
        Self::with_fallback(script, PoolFault::Ready)
    }

    /// A pool that follows `script`, then takes `fallback` on every later checkout.
    pub fn with_fallback(script: Vec<PoolFault>, fallback: PoolFault) -> Self {
        Self {
            script: Mutex::new(script.into()),
            fallback,
            attempts: AtomicU64::new(0),
            successes: AtomicU64::new(0),
        }
    }

    /// A pool that hands out the same fault forever.
    pub fn always(fault: PoolFault) -> Self {
        Self::with_fallback(Vec::new(), fault)
    }

    /// The next checkout, classified by the production boundary.
    ///
    /// On a fault it runs `DbFailure::from_sqlx` — the one place production turns a driver error into the failure
    /// taxonomy — and returns that result unchanged. The `operation` label travels through exactly as it does on the
    /// real acquire path.
    pub fn acquire(&self, operation: &'static str) -> DbResult<()> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let fault = self
            .script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
            .unwrap_or(self.fallback);
        match fault.driver_error() {
            None => {
                self.successes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            Some(error) => Err(DbFailure::from_sqlx(operation, &error)),
        }
    }

    /// How many checkouts were requested.
    pub fn attempts(&self) -> u64 {
        self.attempts.load(Ordering::SeqCst)
    }

    /// How many checkouts handed out a connection.
    pub fn successes(&self) -> u64 {
        self.successes.load(Ordering::SeqCst)
    }

    /// How many scripted faults remain.
    pub fn remaining(&self) -> usize {
        self.script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::DbFailureKind;

    #[test]
    fn the_pool_timeout_is_the_drivers_own_error() {
        let error = PoolFault::PoolTimedOut.driver_error().expect("a timeout");
        assert_eq!(
            error.to_string(),
            "pool timed out while waiting for an open connection"
        );
    }

    #[test]
    fn acquire_classifies_through_the_production_boundary() {
        let harness = DbPoolFaultHarness::scripted(vec![PoolFault::PoolTimedOut, PoolFault::Ready]);
        let failure = harness
            .acquire("db.acquire")
            .expect_err("the first checkout times out");
        assert_eq!(failure.kind, DbFailureKind::Timeout);
        assert_eq!(failure.operation, "db.acquire");
        assert!(harness.acquire("db.acquire").is_ok());
        assert_eq!(harness.attempts(), 2);
        assert_eq!(harness.successes(), 1);
        assert_eq!(harness.remaining(), 0);
    }

    #[test]
    fn a_work_failure_is_not_a_timeout() {
        let harness = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
        let failure = harness
            .acquire("db.acquire")
            .expect_err("a constraint fails");
        assert_eq!(failure.kind, DbFailureKind::Constraint);
        assert!(!failure.retryable);
    }
}
