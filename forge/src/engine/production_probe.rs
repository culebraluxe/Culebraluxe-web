//! What production says it is running: the commit stamped into the live build (`GET /api/build-info`).
//!
//! The DevOps lane's receipt is this answer, read by Forge-owned code. Nothing else produced one in the Rust engine:
//! the deploy agent is read-only, so a `deploy` or `production_smoke` model turn could never present a receipt, and
//! every such turn was paid for, re-prompted and held. The stamp is written at deploy time by
//! `scripts/deploy-prod.sh` (`web/src/api/build_info.rs`), so it names the commit that is actually live.

use std::time::Duration;

/// Where production is, as the production smoke reads it (`cli/src/smoke.rs`).
pub const PROD_URL_ENV: &str = "CULEBRALUXE_PROD_URL";
pub const DEFAULT_PROD_URL: &str = "https://www.culebraluxe.com";
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// The production base URL this process would probe.
pub fn production_base_url() -> String {
    std::env::var(PROD_URL_ENV)
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_PROD_URL.to_string())
}

/// `GET {base}/api/build-info` and return its `sha`: a refusal names why (unreachable, not 200, unstamped).
///
/// Run on a thread of its own with a runtime of its own, so a caller that is already inside a runtime cannot be made
/// to block in it.
pub fn fetch_deployed_sha(base: &str) -> Result<String, String> {
    let url = format!("{base}/api/build-info");
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("no runtime for the production probe: {error}"))?;
                runtime.block_on(async {
                    let client = reqwest::Client::builder()
                        .timeout(PROBE_TIMEOUT)
                        .build()
                        .map_err(|error| {
                            format!("no HTTP client for the production probe: {error}")
                        })?;
                    let response = client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|error| format!("{url} is unreachable: {error}"))?;
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .map_err(|error| format!("{url} body unreadable: {error}"))?;
                    if !status.is_success() {
                        return Err(format!("{url} answered {status}"));
                    }
                    deployed_sha_from_build_info(&body).map_err(|why| format!("{url}: {why}"))
                })
            })
            .join()
            .map_err(|_| "the production probe thread panicked".to_string())?
    })
}

/// The `sha` a build-info answer carries, or why it carries none. Pure, so the reading is pinned without a network.
pub fn deployed_sha_from_build_info(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("build-info is not JSON: {error}"))?;
    let sha = value
        .get("sha")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if sha.len() >= 7 && sha.len() <= 64 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        let note = value
            .get("note")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("no build stamp");
        Err(format!("production carries no commit stamp ({note})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamped_build_names_its_commit_and_an_unstamped_one_says_why() {
        assert_eq!(
            deployed_sha_from_build_info(r#"{"ok":true,"sha":"9EA50F3","version":"1"}"#).as_deref(),
            Ok("9ea50f3")
        );
        let unstamped = deployed_sha_from_build_info(
            r#"{"ok":true,"sha":"","note":"set CULEBRALUXE_BUILD_SHA"}"#,
        )
        .unwrap_err();
        assert!(unstamped.contains("CULEBRALUXE_BUILD_SHA"), "{unstamped}");
        assert!(deployed_sha_from_build_info("<html>").is_err());
    }
}
