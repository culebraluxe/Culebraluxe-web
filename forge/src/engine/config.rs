//! Centralized Forge environment configuration.
use std::time::Duration;

use crate::engine::constants::{FORGE_DEFAULT_HEARTBEAT_SECONDS, FORGE_DEFAULT_STALE_MINUTES};

/// Shared wall-clock budget for one role turn, independent of the selected harness.
pub const TURN_CEILING_ENV: &str = "FORGE_TURN_TIMEOUT_MINUTES";
pub const DEFAULT_TURN_CEILING_MINUTES: u64 = 120;

/// Parse the configured per-turn ceiling. Unset or invalid values use the default; explicit
/// `0`/`off`/`none` means unbounded. Both the job supervisor and execution adapters share this rule.
pub fn turn_ceiling(raw: Option<&str>) -> Option<Duration> {
    let minutes = match raw.map(str::trim) {
        None | Some("") => DEFAULT_TURN_CEILING_MINUTES,
        Some(word)
            if word == "0"
                || word.eq_ignore_ascii_case("off")
                || word.eq_ignore_ascii_case("none") =>
        {
            return None;
        }
        Some(word) => word.parse::<u64>().unwrap_or(DEFAULT_TURN_CEILING_MINUTES),
    };
    Some(Duration::from_secs(minutes * 60))
}

/// Configuration for the Forge worker pass (the scheduler entry point).
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// How many different stories one pass may run concurrently.
    /// Clamped to 1..=8. Default 4.
    pub story_worker_concurrency: usize,

    /// Minutes after which a worker's claim is considered stale.
    /// Default 10 minutes.
    pub stale_after_minutes: i64,

    /// Heartbeat interval in seconds.
    /// Always clamped to be strictly less than `stale_after_minutes * 60`.
    /// Default: stale_window / 4, minimum 15s.
    pub heartbeat_interval: Duration,

    /// Whether an attended (human-supervised) run is allowed.
    /// Set `FORGE_ATTENDED=1` to bypass execution_policy checks.
    pub attended_override: bool,
}

impl WorkerConfig {
    /// Read all worker configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            story_worker_concurrency: story_worker_concurrency(),
            stale_after_minutes: stale_after_minutes(),
            heartbeat_interval: Duration::from_secs(heartbeat_seconds()),
            attended_override: attended_override(),
        }
    }
}

/// Configuration for a child Forge engine process (spawned by the worker).
#[derive(Debug, Clone)]
pub struct ChildConfig {
    /// Minimum pool size for child processes. Default 0 (no warm floor).
    pub db_pool_min: u32,

    /// Maximum pool size for child processes. Default 6.
    pub db_pool_max: u32,

    /// Whether the child may publish to origin/main.
    /// Default true. Set `FORGE_ALLOW_PUBLISH=0` to disable.
    pub allow_publish: bool,

    /// Model turns ceiling per generation. Read from `FORGE_MAX_MODEL_TURNS_PER_GENERATION`.
    /// Default 100. Clamped to 1..=1000.
    pub max_model_turns_per_generation: u32,
}

impl ChildConfig {
    /// Read all child configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            db_pool_min: child_db_pool_min(),
            db_pool_max: child_db_pool_max(),
            allow_publish: allow_publish(),
            max_model_turns_per_generation: max_model_turns_per_generation(),
        }
    }
}

/// Configuration for the Forge routing brain selection.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Routing brain to use. Default is the Rust engine.
    /// `FORGE_ROUTING_BRAIN=reducer` is retired for unattended execution.
    pub brain: String,
}

impl RoutingConfig {
    pub fn from_env() -> Self {
        Self {
            brain: std::env::var("FORGE_ROUTING_BRAIN")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "rust".to_string()),
        }
    }
}

/// Pure: clamp story concurrency to 1..=8, default 4.
fn story_worker_concurrency() -> usize {
    std::env::var("FORGE_STORY_WORKERS")
        .ok()
        .as_deref()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(4)
        .clamp(1, 8)
}

/// Pure: parse stale window in minutes, default from constants.
fn stale_after_minutes() -> i64 {
    std::env::var("AGENT_WORKER_STALE_AFTER_MINUTES")
        .ok()
        .as_deref()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(FORGE_DEFAULT_STALE_MINUTES)
        .max(1)
}

/// Pure: heartbeat interval in seconds, always strictly inside the stale window.
/// Default = window / 4, minimum 15s (but also bounded by FORGE_DEFAULT_HEARTBEAT_SECONDS).
fn heartbeat_seconds() -> u64 {
    let window = stale_after_minutes().max(1) as u64 * 60;
    let default = (window / 4).max(15).min(FORGE_DEFAULT_HEARTBEAT_SECONDS);
    std::env::var("AGENT_WORKER_HEARTBEAT_SECONDS")
        .ok()
        .as_deref()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&seconds| seconds > 0 && seconds < window)
        .unwrap_or(default)
}

/// Pure: attended override check.
fn attended_override() -> bool {
    std::env::var("FORGE_ATTENDED").ok().as_deref() == Some("1")
}

/// Pure: child DB pool min, default 0.
fn child_db_pool_min() -> u32 {
    std::env::var("FORGE_CHILD_DB_POOL_MIN")
        .ok()
        .as_deref()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

/// Pure: child DB pool max, default 6.
fn child_db_pool_max() -> u32 {
    std::env::var("FORGE_CHILD_DB_POOL_MAX")
        .ok()
        .as_deref()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(6)
}

/// Pure: allow publish, default true.
fn allow_publish() -> bool {
    std::env::var("FORGE_ALLOW_PUBLISH")
        .ok()
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or("1")
        != "0"
}

/// Pure: max model turns per generation, default 100, clamped 1..=1000.
fn max_model_turns_per_generation() -> u32 {
    std::env::var("FORGE_MAX_MODEL_TURNS_PER_GENERATION")
        .ok()
        .as_deref()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(100)
        .clamp(1, 1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn turn_ceiling_has_one_harness_neutral_policy() {
        assert_eq!(turn_ceiling(None), Some(Duration::from_secs(120 * 60)));
        assert_eq!(turn_ceiling(Some("45")), Some(Duration::from_secs(45 * 60)));
        assert_eq!(
            turn_ceiling(Some("garbage")),
            Some(Duration::from_secs(120 * 60))
        );
        for unbounded in ["0", "off", "none"] {
            assert_eq!(turn_ceiling(Some(unbounded)), None);
        }
    }

    #[test]
    fn story_concurrency_clamped() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("FORGE_STORY_WORKERS", "64");
        assert_eq!(story_worker_concurrency(), 8);
        std::env::set_var("FORGE_STORY_WORKERS", "0");
        assert_eq!(story_worker_concurrency(), 4);
        std::env::remove_var("FORGE_STORY_WORKERS");
        assert_eq!(story_worker_concurrency(), 4);
    }

    #[test]
    fn heartbeat_inside_window() {
        let _guard = ENV_LOCK.lock().unwrap();
        for stale in [1, 5, 10, 60] {
            std::env::set_var("AGENT_WORKER_STALE_AFTER_MINUTES", stale.to_string());
            std::env::remove_var("AGENT_WORKER_HEARTBEAT_SECONDS");
            let hb = heartbeat_seconds();
            let window = stale as u64 * 60;
            assert!(hb < window, "hb={hb} < window={window} for stale={stale}");
            assert!(hb >= 15, "minimum 15s");
        }
    }

    #[test]
    fn heartbeat_override_clamped() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("AGENT_WORKER_STALE_AFTER_MINUTES", "10");
        std::env::set_var("AGENT_WORKER_HEARTBEAT_SECONDS", "3600"); // 1h > 600s window
        assert_eq!(heartbeat_seconds(), 150); // falls back to default
    }

    #[test]
    fn child_config_defaults() {
        std::env::remove_var("FORGE_CHILD_DB_POOL_MIN");
        std::env::remove_var("FORGE_CHILD_DB_POOL_MAX");
        std::env::remove_var("FORGE_ALLOW_PUBLISH");
        std::env::remove_var("FORGE_MAX_MODEL_TURNS_PER_GENERATION");

        let cfg = ChildConfig::from_env();
        assert_eq!(cfg.db_pool_min, 0);
        assert_eq!(cfg.db_pool_max, 6);
        assert!(cfg.allow_publish);
        assert_eq!(cfg.max_model_turns_per_generation, 100);
    }

    #[test]
    fn worker_config_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("FORGE_STORY_WORKERS");
        std::env::remove_var("AGENT_WORKER_STALE_AFTER_MINUTES");
        std::env::remove_var("AGENT_WORKER_HEARTBEAT_SECONDS");
        std::env::remove_var("FORGE_ATTENDED");

        let cfg = WorkerConfig::from_env();
        assert_eq!(cfg.story_worker_concurrency, 4);
        assert_eq!(cfg.stale_after_minutes, FORGE_DEFAULT_STALE_MINUTES);
        assert_eq!(
            cfg.heartbeat_interval,
            Duration::from_secs(FORGE_DEFAULT_HEARTBEAT_SECONDS)
        );
        assert!(!cfg.attended_override);
    }
}
