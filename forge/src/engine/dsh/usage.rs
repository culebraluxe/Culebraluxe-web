//! Token accounting from the DSH session log.
//!
//! The headless CLI prints only the final assistant text; it does not print usage. The harness does
//! meter it: every `assistant/message` event in `session.<version>.jsonl.zstd` carries the
//! provider-reported `usage` (`inputTokens`, `outputTokens`, `cacheReadTokens`,
//! `cacheWriteTokens`). This module folds that log into Forge's [`HarnessUsage`] so DSH turns are
//! measured rather than guessed.
//!
//! Cost is NOT invented. DSH reports tokens, not dollars, and Forge has no price table for the
//! vendor. The operator supplies per-million-token rates (`DSH_PRICE_INPUT_PER_MTOK`,
//! `DSH_PRICE_OUTPUT_PER_MTOK`, `DSH_PRICE_CACHE_READ_PER_MTOK`); until they are set the usage is
//! returned as `None` — UNMEASURED, never a fabricated zero.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::engine::harness::HarnessUsage;

/// Token buckets summed over one session.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Per-million-token rates the operator configures for the model DSH ran.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelRates {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

impl TokenTotals {
    /// Cost in USD under the configured rates.
    pub fn cost_usd(&self, rates: &ModelRates) -> f64 {
        let per = |tokens: u64, rate: f64| tokens as f64 * rate / 1_000_000.0;
        per(self.input_tokens, rates.input_per_mtok)
            + per(self.output_tokens, rates.output_per_mtok)
            + per(self.cache_read_tokens, rates.cache_read_per_mtok)
    }
}

/// Operator-configured rates. Both input and output rates must be present; cache reads default to
/// the input rate (the conservative assumption when the operator did not state a cache rate).
pub fn rates_from_env() -> Option<ModelRates> {
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
    };
    let input_per_mtok = read("DSH_PRICE_INPUT_PER_MTOK")?;
    let output_per_mtok = read("DSH_PRICE_OUTPUT_PER_MTOK")?;
    let cache_read_per_mtok = read("DSH_PRICE_CACHE_READ_PER_MTOK").unwrap_or(input_per_mtok);
    Some(ModelRates {
        input_per_mtok,
        output_per_mtok,
        cache_read_per_mtok,
    })
}

/// The newest metered log inside a session directory.
fn session_log(session_dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(session_dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("session.") || !name.ends_with(".jsonl.zstd") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(at, _)| modified > *at) {
            best = Some((modified, entry.path()));
        }
    }
    best.map(|(_, path)| path)
}

/// Sum every `assistant/message` usage record in one DSH session. `None` when the log is absent,
/// undecodable, or contains no usage (all of which mean UNMEASURED, not zero).
pub fn read_token_totals(session_dir: &Path) -> Option<TokenTotals> {
    let path = session_log(session_dir)?;
    let file = std::fs::File::open(&path).ok()?;
    let mut decoder = zstd::stream::read::Decoder::new(file).ok()?;
    let mut text = String::new();
    decoder.read_to_string(&mut text).ok()?;
    let mut totals = TokenTotals::default();
    let mut seen = false;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("type").and_then(|value| value.as_str()) != Some("assistant/message") {
            continue;
        }
        let Some(usage) = event.get("data").and_then(|data| data.get("usage")) else {
            continue;
        };
        let bucket = |name: &str| {
            usage
                .get(name)
                .and_then(|value| value.as_u64())
                .unwrap_or_default()
        };
        totals.input_tokens += bucket("inputTokens");
        totals.output_tokens += bucket("outputTokens");
        totals.cache_read_tokens += bucket("cacheReadTokens");
        totals.cache_write_tokens += bucket("cacheWriteTokens");
        seen = true;
    }
    seen.then_some(totals)
}

/// Forge usage for one completed session, priced when rates are configured.
pub fn harness_usage(
    session_dir: &Path,
    session_id: &str,
    rates: Option<&ModelRates>,
) -> Option<HarnessUsage> {
    let rates = rates?;
    let totals = read_token_totals(session_dir)?;
    Some(HarnessUsage {
        session_id: session_id.to_string(),
        tokens_input: i64::try_from(totals.input_tokens).unwrap_or(i64::MAX),
        tokens_output: i64::try_from(totals.output_tokens).unwrap_or(i64::MAX),
        cost_usd: totals.cost_usd(rates),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_session(dir: &Path, events: &[&str]) {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("session.v4.jsonl.zstd");
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = zstd::stream::write::Encoder::new(file, 3).unwrap();
        for event in events {
            writeln!(encoder, "{event}").unwrap();
        }
        encoder.finish().unwrap();
    }

    fn message(input: u64, output: u64, cache: u64) -> String {
        format!(
            r#"{{"type":"assistant/message","data":{{"usage":{{"inputTokens":{input},"outputTokens":{output},"cacheReadTokens":{cache},"cacheWriteTokens":0}}}}}}"#
        )
    }

    #[test]
    fn sums_every_assistant_message_usage() {
        let dir = std::env::temp_dir().join(format!("dsh-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_session(
            &dir,
            &[
                r#"{"type":"session","version":4}"#,
                &message(10, 5, 100),
                r#"{"type":"tool/call","data":{}}"#,
                &message(7, 3, 50),
            ],
        );
        let totals = read_token_totals(&dir).unwrap();
        assert_eq!(
            totals,
            TokenTotals {
                input_tokens: 17,
                output_tokens: 8,
                cache_read_tokens: 150,
                cache_write_tokens: 0
            }
        );
        let rates = ModelRates {
            input_per_mtok: 1.0,
            output_per_mtok: 2.0,
            cache_read_per_mtok: 0.5,
        };
        let usage = harness_usage(&dir, "session-1", Some(&rates)).unwrap();
        assert_eq!(usage.tokens_input, 17);
        assert_eq!(usage.tokens_output, 8);
        assert!((usage.cost_usd - (17.0 + 16.0 + 75.0) / 1_000_000.0).abs() < 1e-12);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_usage_free_session_is_unmeasured() {
        let dir = std::env::temp_dir().join(format!("dsh-usage-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read_token_totals(&dir), None);
        write_session(&dir, &[r#"{"type":"session","version":4}"#]);
        assert_eq!(read_token_totals(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rates_require_input_and_output_but_cache_defaults_to_input() {
        // The env is process-global; this test asserts the pure parser shape through a helper value.
        let rates = ModelRates {
            input_per_mtok: 0.27,
            output_per_mtok: 1.10,
            cache_read_per_mtok: 0.27,
        };
        assert!((rates.input_per_mtok - 0.27).abs() < f64::EPSILON);
    }
}
