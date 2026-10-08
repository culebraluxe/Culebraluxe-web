//! INT.MUX — configuration boundary (TST-INT-MUX-001).
//!
//! Contract: one Mux account for every environment. `MuxConfig::from_env`
//! reads `MUX_TOKEN_ID` / `MUX_TOKEN_SECRET` (or their `_PROD`-suffixed
//! twins — the same pair on DEV and in production), trims and defaults the
//! rest (base URL, timeout), and refuses with a naming error when the pair
//! is absent. The video routes build their client from exactly this read, so
//! a misconfigured process fails at the boundary with a name, not mid-call
//! with a confusing provider error.
//!
//! Level: L1 Component — the production config reader with the process
//! environment under test control. No database, no network, no live Mux.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_mux__001__configuration_boundary

use apis::mux::{MuxClient, MuxConfig};
use test_harness::source;

const KEYS: [&str; 5] = [
    "MUX_TOKEN_ID",
    "MUX_TOKEN_SECRET",
    "MUX_TOKEN_ID_PROD",
    "MUX_TOKEN_SECRET_PROD",
    "MUX_BASE_URL",
];

/// Remove every Mux config key, returning what was there so the test restores
/// the process environment on the way out.
fn strip_config() -> Vec<(&'static str, Option<String>)> {
    KEYS.into_iter()
        .map(|key| {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            (key, previous)
        })
        .collect()
}

fn restore_config(previous: Vec<(&'static str, Option<String>)>) {
    for (key, value) in previous {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}

fn set_pair(id: &str, secret: &str) {
    std::env::set_var("MUX_TOKEN_ID", id);
    std::env::set_var("MUX_TOKEN_SECRET", secret);
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn int_mux_001__configuration_boundary() {
    let previous = strip_config();
    std::env::remove_var("MUX_TIMEOUT_MS");

    // Positive (refusal): with no credentials configured, the boundary names
    // the missing key instead of building a client that fails mid-call.
    let error = MuxConfig::from_env().expect_err("no credentials, no config");
    assert!(
        error.contains("MUX_TOKEN_ID"),
        "the refusal names the missing credential: {error}"
    );

    // Positive: the pair configures, with production defaults for the rest.
    set_pair("test-id", "test-secret");
    let config = MuxConfig::from_env().expect("the pair configures");
    assert_eq!(config.token_id, "test-id");
    assert_eq!(config.token_secret, "test-secret");
    assert_eq!(config.base_url, "https://api.mux.com/video/v1");
    assert_eq!(config.timeout_ms, 20_000);

    // Positive: surrounding whitespace and a trailing slash never reach the
    // client — the read normalizes what operators paste.
    std::env::set_var("MUX_TOKEN_ID", "  test-id  ");
    std::env::set_var("MUX_BASE_URL", "https://custom.example/mux/");
    let config = MuxConfig::from_env().expect("padded values configure");
    assert_eq!(config.token_id, "test-id");
    assert_eq!(config.base_url, "https://custom.example/mux");

    // Positive: one account for every environment — the `_PROD`-suffixed
    // twins satisfy the same read when the bare keys are absent.
    strip_config();
    std::env::set_var("MUX_TOKEN_ID_PROD", "prod-id");
    std::env::set_var("MUX_TOKEN_SECRET_PROD", "prod-secret");
    let config = MuxConfig::from_env().expect("the _PROD twins configure");
    assert_eq!(config.token_id, "prod-id");

    // Positive: the timeout is honored when set and defaulted when senseless.
    set_pair("test-id", "test-secret");
    std::env::set_var("MUX_TIMEOUT_MS", "5000");
    assert_eq!(
        MuxConfig::from_env().expect("custom timeout").timeout_ms,
        5_000
    );
    for bad in ["0", "not-a-number", ""] {
        std::env::set_var("MUX_TIMEOUT_MS", bad);
        assert_eq!(
            MuxConfig::from_env()
                .expect("bad timeout defaults")
                .timeout_ms,
            20_000,
            "timeout {bad:?} falls back to the default"
        );
    }
    std::env::remove_var("MUX_TIMEOUT_MS");

    // Positive: a configured client builds offline — construction is local,
    // the network only happens on a call.
    set_pair("test-id", "test-secret");
    let config = MuxConfig::from_env().expect("the pair configures");
    assert!(
        MuxClient::new(config).is_ok(),
        "a configured client builds without touching the network"
    );

    // Negative: whitespace-only credentials are missing credentials — a blank
    // paste must not satisfy the boundary.
    std::env::set_var("MUX_TOKEN_ID", "   ");
    std::env::remove_var("MUX_TOKEN_ID_PROD");
    let error = MuxConfig::from_env().expect_err("a blank id is no id");
    assert!(
        error.contains("MUX_TOKEN_ID"),
        "blank refused by name: {error}"
    );

    // Structural: the video routes build their client from this same read —
    // the test exercises the boundary production uses, not a parallel one.
    let bodies = source::read(&source::workspace_root().join("web/src/api/routes/bodies.rs"));
    assert!(
        bodies.contains("MuxConfig::from_env()"),
        "the video routes read MuxConfig::from_env"
    );

    restore_config(previous);
    std::env::remove_var("MUX_TIMEOUT_MS");
}
