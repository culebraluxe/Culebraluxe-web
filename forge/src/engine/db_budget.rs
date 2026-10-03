//! The database budgets an ENGINE process runs under, installed once at startup.
//!
//! The pool's defaults are sized for the request path (`db/src/pool.rs`): 30 seconds for any statement
//! ("a ceiling against a stuck query, not a performance budget") and 10 seconds to wait for an open connection,
//! with a floor of five connections held warm. Both are right for a page load and wrong for an engine, whose first
//! statement may be the one that wakes a suspended Neon branch and touches a table's cold pages.
//!
//! Measured 2026-09-29, on the scheduler's own ticks against PROD, with an engine that had been failing silently
//! for reasons nobody could see in one line of output:
//!
//! - `error returned from database: canceling statement due to statement timeout` (SQLSTATE 57014) killed a run
//!   `~55s` in — the whole run lost, no receipt, the story left `In Progress`.
//! - `Timeout during db.connect (incident …): pool timed out while waiting for an open connection` ended a tick
//!   after 12 seconds with nothing claimed and no story run at all.
//!
//! The identical run with a raised statement ceiling went on to dispatch its first role turn, which is what makes
//! these budgets rather than symptoms. An explicit environment value always wins: this installs defaults, it does
//! not override a declaration. Both binaries that talk to the control plane (`forge`, `forge-worker`) install
//! these, because the failing process was the worker, not the runner — the two must not disagree.

use std::env;

/// The engine's per-statement ceiling: five minutes.
pub const ENGINE_STATEMENT_TIMEOUT_MS: &str = "300000";

/// The engine's wait for an open connection: sixty seconds, long enough for a cold branch's handshakes.
pub const ENGINE_CONNECT_TIMEOUT_MS: &str = "60000";

/// What an engine process is actually using, so a run can say it rather than leave it to be inferred. The run that
/// died at 30 seconds left no trace of which ceiling it had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDbBudget {
    pub statement_timeout_ms: String,
    pub connect_timeout_ms: String,
}

/// `declared` if the process declared one, else the engine's default. An empty or whitespace value counts as not
/// declared — an environment variable set to `""` is a mistake, not a ceiling of zero.
pub fn resolve_engine_budget(declared: Option<&str>, default: &str) -> String {
    match declared {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => default.to_string(),
    }
}

/// Install the engine's database budgets for this process and report what it is using.
pub fn install_engine_db_budget() -> EngineDbBudget {
    let statement_timeout_ms = resolve_engine_budget(
        env::var("FORGE_DB_STATEMENT_TIMEOUT_MS").ok().as_deref(),
        ENGINE_STATEMENT_TIMEOUT_MS,
    );
    let connect_timeout_ms = resolve_engine_budget(
        env::var("FORGE_DB_POOL_CONNECT_MS").ok().as_deref(),
        ENGINE_CONNECT_TIMEOUT_MS,
    );
    env::set_var("FORGE_DB_STATEMENT_TIMEOUT_MS", &statement_timeout_ms);
    env::set_var("FORGE_DB_POOL_CONNECT_MS", &connect_timeout_ms);
    EngineDbBudget {
        statement_timeout_ms,
        connect_timeout_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_value_wins_over_the_engines_default() {
        assert_eq!(
            resolve_engine_budget(Some("15000"), ENGINE_STATEMENT_TIMEOUT_MS),
            "15000"
        );
    }

    #[test]
    fn an_absent_or_empty_value_takes_the_engines_default() {
        assert_eq!(
            resolve_engine_budget(None, ENGINE_STATEMENT_TIMEOUT_MS),
            ENGINE_STATEMENT_TIMEOUT_MS
        );
        // An empty variable is a mistake, not a ceiling of zero: it must not disable the ceiling.
        assert_eq!(
            resolve_engine_budget(Some(""), ENGINE_CONNECT_TIMEOUT_MS),
            ENGINE_CONNECT_TIMEOUT_MS
        );
        assert_eq!(
            resolve_engine_budget(Some("   "), ENGINE_CONNECT_TIMEOUT_MS),
            ENGINE_CONNECT_TIMEOUT_MS
        );
    }

    #[test]
    fn the_engines_defaults_are_longer_than_the_request_paths() {
        // The pool's defaults, quoted from db/src/pool.rs: DEFAULT_STATEMENT_TIMEOUT_MS = 30_000 and
        // FORGE_DB_POOL_CONNECT_MS = 10_000. These are the two budgets that killed runs; if either engine default
        // ever drops to the request path's, the failure mode comes back.
        let statement: u64 = ENGINE_STATEMENT_TIMEOUT_MS.parse().expect("a number");
        let connect: u64 = ENGINE_CONNECT_TIMEOUT_MS.parse().expect("a number");
        assert!(statement > 30_000, "statement ceiling must exceed 30s");
        assert!(connect > 10_000, "connect budget must exceed 10s");
    }
}
