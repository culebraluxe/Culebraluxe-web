//! Forge-wide constants.
//!
//! Centralized constants used across the Forge engine to avoid drift.

/// Default stale threshold in minutes for Forge claims and jobs.
/// 
/// Used by:
/// - `engine::stale_claim::stale_after_ms_from_env`
/// - `engine::config::stale_after_minutes`
/// - `cli::forge::reset::DEFAULT_CLEAN_STALE_MINUTES`
/// 
/// Can be overridden via environment variables:
/// - `AGENT_WORKER_STALE_AFTER_MINUTES` (engine)
/// - `FORGE_STALE_MINUTES` (config)
/// - `--stale-minutes` flag (cli)
pub const FORGE_DEFAULT_STALE_MINUTES: i64 = 15;

/// Default stale threshold in milliseconds.
pub const FORGE_DEFAULT_STALE_MS: i64 = FORGE_DEFAULT_STALE_MINUTES * 60_000;

/// Default heartbeat interval in seconds (1/4 of stale window, minimum 15s).
/// 15 min * 60 / 4 = 225 seconds, max with 15 = 225.
pub const FORGE_DEFAULT_HEARTBEAT_SECONDS: u64 = 225;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_consistent() {
        assert_eq!(FORGE_DEFAULT_STALE_MINUTES, 15);
        assert_eq!(FORGE_DEFAULT_STALE_MS, 15 * 60_000);
        assert_eq!(FORGE_DEFAULT_HEARTBEAT_SECONDS, 15 * 60 / 4);
    }
}