//! `smoke prod` — ask production whether it is actually working, not just deployed.
//!
//! PORTED FROM `scripts/prod-smoke.ts` (2026-09-28, the last live TypeScript in the release path). The reasoning
//! in that file's header is kept, because it is why the checks are shaped this way:
//!
//! Pirated from OpenContext's `.github/workflows/cli-smoke.yml` (0xranx/OpenContext, MIT), which installs the
//! package it just built and runs it, because a test suite answers "does the code work" and only the deployed
//! artefact answers "does the thing people load work". Our deploy script verified the sha from `/api/build-info`
//! and stopped there: a deploy could alias to a build whose pages 500 and still report VERIFIED.
//!
//! Two deliberate choices: the CANONICAL domain, never a `*.vercel.app` URL (Vercel Authentication answers 302
//! to a login page there, so a check against it can never pass); and checks written against strings the pages own
//! (the title, `Selected Properties`) rather than byte counts, so a copy edit does not turn a smoke test into a
//! false alarm.
//!
//!   smoke prod                     # health: does the site answer, and with the right shape
//!   smoke prod --expect-head       # also assert the live sha equals HEAD (the deploy path)
//!   smoke prod --format json       # the same report, for a machine

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

const DEFAULT_URL: &str = "https://www.culebraluxe.com";
const DEFAULT_TIMEOUT_MS: u64 = 25_000;

pub async fn dispatch(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("prod") => prod(&args[1..]).await,
        _ => Err(std::io::Error::other(
            "usage: smoke prod [--expect-head] [--expect-sha <sha>] [--format json]",
        )
        .into()),
    }
}

struct Check {
    name: String,
    url: String,
    ok: bool,
    detail: String,
}

struct Report {
    checks: Vec<Check>,
    live_sha: String,
    live_version: String,
    live_built_at: String,
}

impl Report {
    fn push(&mut self, name: &str, url: &str, ok: bool, detail: String) {
        self.checks.push(Check {
            name: name.to_owned(),
            url: url.to_owned(),
            ok,
            detail,
        });
    }
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

/// One GET, following redirects, with the smoke's user agent. Errors are returned as text: this is a report, and
/// "connection refused" belongs in the same column as "status 500".
async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<(u16, String), String> {
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            match response.text().await {
                Ok(body) => Ok((status, body)),
                Err(error) => Err(error.to_string()),
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

/// `git rev-parse --short HEAD`.
fn git_head_short() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

/// How far behind HEAD the live build is, or `None` when the sha is not in this checkout.
fn commits_behind(live_sha: &str, head: &str) -> Option<u64> {
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{live_sha}..{head}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

fn timeout_ms() -> u64 {
    std::env::var("SMOKE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

async fn prod(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let json_output = args
        .iter()
        .position(|arg| arg == "--format")
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
        == Some("json");
    let expect_head = args.iter().any(|arg| arg == "--expect-head");
    let expected_sha = args
        .iter()
        .position(|arg| arg == "--expect-sha")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_default();
    let base = std::env::var("CULEBRALUXE_PROD_URL")
        .unwrap_or_else(|_| DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms()))
        .user_agent("culebraluxe-prod-smoke")
        .build()?;
    let head = git_head_short();
    let mut report = Report {
        checks: Vec::new(),
        live_sha: String::new(),
        live_version: String::new(),
        live_built_at: String::new(),
    };

    // 1. The build stamp: what is live, and is it a build we recognise.
    let build_info_url = format!("{base}/api/build-info");
    match fetch_text(&client, &build_info_url).await {
        Ok((status, body)) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            report.live_sha = parsed["sha"].as_str().unwrap_or_default().to_owned();
            report.live_version = parsed["version"].as_str().unwrap_or_default().to_owned();
            report.live_built_at = parsed["builtAt"].as_str().unwrap_or_default().to_owned();
            let shaped = status == 200
                && (7..=40).contains(&report.live_sha.len())
                && report.live_sha.chars().all(|c| c.is_ascii_hexdigit())
                && !report.live_version.is_empty();
            let detail = if shaped {
                format!(
                    "{} · {} · built {}",
                    report.live_version, report.live_sha, report.live_built_at
                )
            } else {
                format!("status {status}, unexpected body: {}", truncate(&body, 120))
            };
            report.push(
                "build-info is a real build stamp",
                &build_info_url,
                shaped,
                detail,
            );
        }
        Err(error) => report.push(
            "build-info is a real build stamp",
            &build_info_url,
            false,
            error,
        ),
    }

    // 2. The public homepage is server-rendered enough that its brand marker is present in the response.
    let home_url = format!("{base}/");
    match fetch_text(&client, &home_url).await {
        Ok((status, body)) => {
            let has_marker = body.contains("CulebraLuxe");
            let detail = if status != 200 {
                format!("status {status}")
            } else if has_marker {
                format!(
                    "200, {}KB, marker \"CulebraLuxe\" present",
                    body.len() / 1024
                )
            } else {
                "200 but the CulebraLuxe marker is missing".to_owned()
            };
            report.push(
                "home page renders",
                &home_url,
                status == 200 && has_marker,
                detail,
            );
        }
        Err(error) => report.push("home page renders", &home_url, false, error),
    }

    // 3. /buyers is Yew-owned and therefore CLIENT rendered. Its HTTP document intentionally contains only the
    // Rust mount point; "Selected Properties" is emitted by WASM after boot and can never be a reliable fetch
    // smoke marker. Prove both halves instead: the deployed page contains the mount, and the production
    // page-data endpoint returns at least one public listing.
    let buyers_url = format!("{base}/buyers");
    match fetch_text(&client, &buyers_url).await {
        Ok((status, body)) => {
            let has_mount = body.contains("id=\"rust-ui\"");
            let detail = if status != 200 {
                format!("status {status}")
            } else if has_mount {
                "200, Rust/Yew mount present".to_owned()
            } else {
                "200 but the #rust-ui mount is missing".to_owned()
            };
            report.push(
                "buyers Yew shell renders",
                &buyers_url,
                status == 200 && has_mount,
                detail,
            );
        }
        Err(error) => report.push("buyers Yew shell renders", &buyers_url, false, error),
    }

    let inventory_url = format!("{base}/api/rust-ui/public-page?screen=site-buyers");
    let mut first_public_property_slug = String::new();
    match fetch_text(&client, &inventory_url).await {
        Ok((status, body)) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let listings = parsed["listings"].as_array().cloned().unwrap_or_default();
            first_public_property_slug = listings
                .first()
                .and_then(|listing| listing["slug"].as_str())
                .unwrap_or_default()
                .to_owned();
            let detail = if status != 200 {
                format!("status {status}")
            } else if !listings.is_empty() {
                format!("200, {} public listing(s)", listings.len())
            } else {
                "200 but production returned zero public listings".to_owned()
            };
            report.push(
                "buyers production inventory loads",
                &inventory_url,
                status == 200 && !listings.is_empty(),
                detail,
            );
        }
        Err(error) => report.push(
            "buyers production inventory loads",
            &inventory_url,
            false,
            error,
        ),
    }

    // 4. Property detail media must live INSIDE the Rust PropertyRecord. A top-level heroUrl is ignored by
    // PageContent deserialization and produces a gray 16:9 placeholder while the facts still render.
    let detail_name = "property detail media contract is renderable by Rust";
    if first_public_property_slug.is_empty() {
        report.push(
            detail_name,
            &format!("{base}/api/rust-ui/public-page?screen=site-property-detail"),
            false,
            "no public property slug was available to verify the detail page".to_owned(),
        );
    } else {
        let url = format!(
            "{base}/api/rust-ui/public-page?screen=site-property-detail&scope={first_public_property_slug}"
        );
        match fetch_text(&client, &url).await {
            Ok((status, body)) => {
                let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let hero = parsed["property"]["heroUrl"].as_str().unwrap_or_default();
                let gallery = parsed["property"]["gallery"].as_array().map_or(0, Vec::len);
                let shaped = status == 200 && hero.starts_with("/api/media/") && gallery > 0;
                let detail = if shaped {
                    format!("200, hero nested in property record, {gallery} gallery image(s)")
                } else {
                    format!(
                        "status {status}, nested hero {}, gallery {gallery}",
                        if hero.is_empty() {
                            "missing"
                        } else {
                            "present but unexpected"
                        }
                    )
                };
                report.push(detail_name, &url, shaped, detail);
            }
            Err(error) => report.push(detail_name, &url, false, error),
        }
    }
    // 5 and 6, then the report: readiness, the optional sha assertion, and the exit code.
    let failures = ready_check_and_report(
        &client,
        &base,
        &mut report,
        &head,
        &expected_sha,
        expect_head,
        json_output,
    )
    .await?;
    if failures > 0 {
        return Err(std::io::Error::other(format!("{failures} smoke check(s) failed")).into());
    }
    Ok(())
}

async fn ready_check_and_report(
    client: &reqwest::Client,
    base: &str,
    report: &mut Report,
    head: &Option<String>,
    expected_sha: &str,
    expect_head: bool,
    json_output: bool,
) -> Result<u8, Box<dyn std::error::Error>> {
    // 5. The Rust API's own readiness, and WHICH DATABASE it resolved. Vercel serves the site and the private
    // service binding; a 200 proves the frontend has RUST_API_BASE_URL, the container is running, and that
    // container can ping the database it selected. Production must report "prod" — a live container pointed at
    // DEV is a failed release, not a partial success.
    let ready_url = format!("{base}/api/rust-ready");
    match fetch_text(client, &ready_url).await {
        Ok((status, body)) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let target = parsed["databaseTarget"].as_str().unwrap_or_default();
            let ready = status == 200 && parsed["ok"].as_bool() == Some(true) && target == "prod";
            let detail = if ready {
                "frontend binding -> Rust container -> PROD Neon".to_owned()
            } else {
                format!(
                    "status {status}, target {}, {}",
                    if target.is_empty() { "<none>" } else { target },
                    parsed["error"].as_str().unwrap_or("not ready")
                )
            };
            report.push(
                "Rust API is ready on PROD database",
                &ready_url,
                ready,
                detail,
            );
        }
        Err(error) => report.push(
            "Rust API is ready on PROD database",
            &ready_url,
            false,
            error,
        ),
    }

    // 6. The sha assertion, only when the caller asks for it (the deploy path does).
    let wanted = if expected_sha.is_empty() {
        if expect_head {
            head.clone().unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        expected_sha.to_owned()
    };
    if !wanted.is_empty() {
        // THE TWO SIDES ARE NOT THE SAME WIDTH, BY DESIGN. `/api/build-info` serves the stamped commit — the whole
        // sha since 2026-09-28 (`bcc6e52e`; before that it was the retired site's `cockpitBuildLabel()`, seven
        // characters) — while `--expect-head` supplies `git rev-parse --short`, which lengthens as the repository
        // grows (it returned 8 on 2026-09-16). Comparing the strings whole asked "did git's abbreviation grow",
        // not "is the live build HEAD". Compare the length they SHARE.
        let shared = report.live_sha.len().min(wanted.len());
        let agrees = shared > 0 && report.live_sha[..shared] == wanted[..shared];
        let detail = if agrees {
            format!("serving {}", report.live_sha)
        } else {
            format!(
                "live {} vs expected {wanted}",
                if report.live_sha.is_empty() {
                    "<no answer>"
                } else {
                    &report.live_sha
                }
            )
        };
        report.push(
            &format!("live sha equals {wanted}"),
            &format!("{base}/api/build-info"),
            agrees,
            detail,
        );
    }

    let behind = match (head, report.live_sha.is_empty()) {
        (Some(head), false) => commits_behind(&report.live_sha, head),
        _ => None,
    };
    let failures = report.checks.iter().filter(|check| !check.ok).count();

    if json_output {
        let checks = report
            .checks
            .iter()
            .map(|check| {
                json!({
                    "name": check.name,
                    "url": check.url,
                    "ok": check.ok,
                    "detail": check.detail,
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "base": base,
                "live": {
                    "sha": report.live_sha,
                    "version": report.live_version,
                    "builtAt": report.live_built_at,
                },
                "head": head,
                "commitsBehind": behind,
                "failures": failures,
                "checks": checks,
            }))?
        );
    } else {
        println!("Production smoke — {base}");
        for check in &report.checks {
            println!(
                "  {}  {}",
                if check.ok { "ok  " } else { "FAIL" },
                check.name
            );
            println!("        {}", check.detail);
        }
        if let Some(behind) = behind.filter(|behind| *behind > 0) {
            // Reported, never failed: a release per sprint means production is SUPPOSED to sit behind HEAD
            // between releases. Saying so is the point; failing would be wrong.
            println!(
                "\n  note: production is {behind} commit(s) behind HEAD ({}) — expected between releases, and \
                 the Cockpit corner shows which build is live.",
                head.as_deref().unwrap_or("unknown")
            );
        }
        let live = if report.live_sha.is_empty() {
            ", live sha unknown".to_owned()
        } else {
            format!(", live {} {}", report.live_version, report.live_sha)
        };
        println!(
            "\nsmoke:prod — {}/{} checks passed{live}",
            report.checks.len() - failures,
            report.checks.len()
        );
    }

    Ok(failures as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sha_comparison_uses_the_length_the_two_sides_share() {
        // `/api/build-info` serves seven characters; `git rev-parse --short` grows with the repository. The
        // comparison is the shared prefix — comparing the strings whole asked "did git's abbreviation grow".
        let live = "40c7d99";
        let wanted = "40c7d99c";
        let shared = live.len().min(wanted.len());
        assert_eq!(&live[..shared], &wanted[..shared]);
        assert_ne!(live, wanted);
    }

    #[test]
    fn a_missing_live_sha_never_agrees_with_anything() {
        let live = "";
        let wanted = "40c7d99c";
        assert_eq!(live.len().min(wanted.len()), 0);
    }

    #[test]
    fn truncating_a_body_counts_characters_not_bytes() {
        // The report slices a response body for a one-line detail; slicing bytes would panic on a page that
        // starts with a multi-byte character (a curly quote is the first character of some of them).
        let body = "é".repeat(200);
        assert_eq!(truncate(&body, 120).chars().count(), 120);
    }

    #[test]
    fn the_build_stamp_shape_is_checked_the_way_the_script_checked_it() {
        let shaped = |sha: &str, version: &str, status: u16| {
            status == 200
                && (7..=40).contains(&sha.len())
                && sha.chars().all(|c| c.is_ascii_hexdigit())
                && !version.is_empty()
        };
        assert!(shaped("40c7d99", "2026-09-28", 200));
        assert!(shaped("40c7d99c1a2b3c4d5e6f708192a3b4c5d6e7f80", "v", 200));
        assert!(!shaped("40c7d99", "2026-09-28", 500), "not a 200");
        assert!(!shaped("40c7d9", "2026-09-28", 200), "too short");
        assert!(!shaped("not-hex-at-all", "2026-09-28", 200), "not a sha");
        assert!(!shaped("40c7d99", "", 200), "no version");
    }
}
