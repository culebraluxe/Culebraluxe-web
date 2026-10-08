//! WHAT IS LIVE, AND WHICH DATABASE IT RESOLVED — the two questions a release gate asks.
//!
//! `GET /api/build-info` and `GET /api/rust-ready` are the addresses the production smoke (`cli/src/smoke.rs`)
//! asks. Both were Vercel functions of the retired TypeScript site and went with the port, so checks 1 and 5 could not
//! pass — but the *answers* were never missing, only unaddressed: readiness is `/readyz` and it already reports the
//! same `ok` + `databaseTarget` (`routes/security_service.rs::ready`), and the DB target has been resolvable from the
//! environment since the pool was written (`db::resolve_declared_target`).
//!
//! The stamp is READ, never invented. `scripts/deploy-prod.sh` writes `CULEBRALUXE_BUILD_SHA` and
//! `CULEBRALUXE_BUILT_AT` into the runtime image at deploy time, so the stamp is the commit that was deployed rather
//! than a compile-time guess. `VERCEL_GIT_COMMIT_SHA` and `GIT_COMMIT_SHA` are honoured for a local or CI run.
//!
//! When a stamp is set but is not a commit sha, this reports the reason in `note` instead of something plausible: an
//! unstamped build and a mis-stamped one must both fail a release check loudly, not pass quietly.

use super::ApiState;
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    ok: bool,
    /// The deployed commit: 7..=40 lowercase hex digits, or empty when nothing was stamped.
    sha: String,
    /// Never empty, even unstamped — the crate version is real either way.
    version: String,
    /// When the stamp was written (RFC3339), or empty.
    built_at: String,
    /// `dev` or `prod`: the database this process actually resolved. AGENTS.md says check it before believing anything.
    database_target: String,
    /// Why a stamp was rejected (or what to set). Absent on the happy path, so a healthy answer stays quiet.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// The release gate's first question. It reports the resolved database beside the stamp, because "which build" and
/// "which database" are one answer to an operator: a live container pointed at DEV is a failed release, not a partial
/// success.
pub async fn build_info(State(state): State<ApiState>) -> Json<BuildInfo> {
    let stamp = resolve_stamp(&[
        (
            "CULEBRALUXE_BUILD_SHA",
            std::env::var("CULEBRALUXE_BUILD_SHA").ok(),
        ),
        (
            "VERCEL_GIT_COMMIT_SHA",
            std::env::var("VERCEL_GIT_COMMIT_SHA").ok(),
        ),
        ("GIT_COMMIT_SHA", std::env::var("GIT_COMMIT_SHA").ok()),
    ]);
    Json(BuildInfo {
        ok: true,
        sha: stamp.sha,
        version: stamp.version,
        built_at: trimmed_env("CULEBRALUXE_BUILT_AT"),
        database_target: state.db().target().as_str().to_owned(),
        note: stamp.note,
    })
}

fn trimmed_env(name: &str) -> String {
    std::env::var(name).unwrap_or_default().trim().to_owned()
}

/// A resolved build stamp: what to serve, and why a candidate was refused.
///
/// TEST SEAM (TST-RUNTIME-DEPLOY-001/002): `pub` so the contract-test harness (`test-harness`) can drive the exact
/// resolution the `build_info` route serves. Visibility only — the route calls this same function, and no behavior
/// changes with the wider visibility.
#[derive(Debug, PartialEq, Eq)]
pub struct Stamp {
    pub sha: String,
    pub version: String,
    pub note: Option<String>,
}

/// Longest abbreviation git itself produces; anything longer is not a commit sha.
///
/// TEST SEAM (TST-RUNTIME-DEPLOY-002): part of the commit-sha contract the harness pins.
pub const MAX_SHA: usize = 40;
/// Shortest abbreviation `git rev-parse --short` returns today, and what `cockpitBuildLabel()` used to serve.
///
/// TEST SEAM (TST-RUNTIME-DEPLOY-002): part of the commit-sha contract the harness pins.
pub const MIN_SHA: usize = 7;

/// Resolve the first stamped candidate that IS a commit sha.
///
/// TEST SEAM (TST-RUNTIME-DEPLOY-001): `pub` so the harness exercises the production resolution (precedence,
/// refusal without fall-through, never-invented stamp) instead of a copy of it.
///
/// Candidates are `(variable name, value)` in precedence order. A candidate that is set but not a sha does not fall
/// through to the next one — that would let a tag in `VERCEL_GIT_COMMIT_SHA` quietly outrank the real stamp in
/// `CULEBRALUXE_BUILD_SHA` — it is refused with the reason named.
pub fn resolve_stamp(candidates: &[(&str, Option<String>)]) -> Stamp {
    let mut refusal: Option<String> = None;
    for (name, value) in candidates {
        let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };
        if let Some(sha) = as_commit_sha(value) {
            return Stamp {
                version: format!("{} ({})", env!("CARGO_PKG_VERSION"), &sha[..MIN_SHA]),
                sha,
                note: None,
            };
        }
        refusal = Some(format!(
            "{name} is not a commit sha: {} character(s), and a commit sha is {MIN_SHA}..={MAX_SHA} hex digits",
            value.chars().count()
        ));
        break;
    }

    Stamp {
        // The crate version is real whether or not anything was stamped, so `version` is never empty: a check that
        // asks "is this a build we recognise" fails on the empty `sha`, which is the honest answer.
        version: env!("CARGO_PKG_VERSION").to_owned(),
        sha: String::new(),
        note: Some(refusal.unwrap_or_else(|| {
            format!(
                "no build stamp: set CULEBRALUXE_BUILD_SHA (or VERCEL_GIT_COMMIT_SHA) to the deployed commit, \
                 {MIN_SHA}..={MAX_SHA} hex digits"
            )
        })),
    }
}

/// Lowercase hex, {MIN_SHA}..={MAX_SHA} digits, and nothing else. Git's abbreviations are already lowercase, so
/// normalising here is what makes the comparison in the smoke exact rather than case-dependent.
fn as_commit_sha(value: &str) -> Option<String> {
    let hex = value.len() >= MIN_SHA
        && value.len() <= MAX_SHA
        && value.chars().all(|c| c.is_ascii_hexdigit());
    hex.then(|| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped(value: &str) -> Stamp {
        resolve_stamp(&[("CULEBRALUXE_BUILD_SHA", Some(value.to_owned()))])
    }

    const FULL: &str = "9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2";

    #[test]
    fn a_full_sha_is_served_and_labelled_with_its_abbreviation() {
        let stamp = stamped(FULL);
        assert_eq!(stamp.sha, FULL);
        assert_eq!(
            stamp.version,
            format!("{} (9ea50f3)", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(stamp.note, None);
    }

    #[test]
    fn a_sha_from_command_substitution_is_trimmed() {
        // `git rev-parse HEAD` inside a shell carries a newline when it is not stripped; an env file may keep it.
        assert_eq!(stamped("9ea50f32\n").sha, "9ea50f32");
        assert_eq!(stamped("  9ea50f32  ").sha, "9ea50f32");
    }

    #[test]
    fn uppercase_hex_is_normalised_so_the_compare_is_exact() {
        assert_eq!(stamped("9EA50F3").sha, "9ea50f3");
    }

    #[test]
    fn the_boundaries_are_seven_and_forty() {
        assert_eq!(stamped("9ea50f3").sha, "9ea50f3");
        assert_eq!(stamped(FULL).sha, FULL);
        assert!(stamped("9ea50f").sha.is_empty(), "6 digits is not a sha");
        assert!(
            stamped(&format!("{FULL}a")).sha.is_empty(),
            "41 digits is not a sha"
        );
    }

    #[test]
    fn an_empty_candidate_does_not_block_the_next() {
        let stamp = resolve_stamp(&[
            ("CULEBRALUXE_BUILD_SHA", Some("  ".to_owned())),
            ("VERCEL_GIT_COMMIT_SHA", Some("9ea50f32".to_owned())),
        ]);
        assert_eq!(stamp.sha, "9ea50f32");
    }

    #[test]
    fn a_tag_is_refused_with_the_reason_instead_of_being_served() {
        // A tag would satisfy nobody: the check wants a commit it can diff against HEAD.
        let stamp = stamped("v1.2.3");
        assert!(stamp.sha.is_empty());
        assert!(!stamp.version.is_empty(), "version is real even unstamped");
        let note = stamp.note.unwrap_or_default();
        assert!(
            note.contains("CULEBRALUXE_BUILD_SHA"),
            "the note names the variable: {note}"
        );
        assert!(
            note.contains("6 character(s)"),
            "the note says what was wrong: {note}"
        );
    }

    #[test]
    fn a_refused_stamp_does_not_fall_through_to_the_next_variable() {
        let stamp = resolve_stamp(&[
            ("CULEBRALUXE_BUILD_SHA", Some("v1.2.3".to_owned())),
            ("VERCEL_GIT_COMMIT_SHA", Some("9ea50f32".to_owned())),
        ]);
        assert!(
            stamp.sha.is_empty(),
            "the tag outranks nothing and hides nothing"
        );
    }

    #[test]
    fn an_unset_stamp_refuses_and_says_which_variable_to_set() {
        let stamp = resolve_stamp(&[("CULEBRALUXE_BUILD_SHA", None)]);
        assert!(stamp.sha.is_empty());
        assert_eq!(stamp.version, env!("CARGO_PKG_VERSION"));
        assert!(stamp
            .note
            .unwrap_or_default()
            .contains("CULEBRALUXE_BUILD_SHA"));
    }

    #[test]
    fn a_stamp_is_never_invented_from_nothing() {
        // The one behaviour that must never regress: no environment, no sha. Not "unknown", not HEAD.
        let stamp = resolve_stamp(&[]);
        assert!(stamp.sha.is_empty());
    }

    #[test]
    fn the_json_keys_are_the_ones_the_release_gate_reads() {
        // The smoke parses `sha`, `version` and `builtAt` out of this body by name (`cli/src/smoke.rs`), and
        // the ready route it calls next reads `databaseTarget`. Renaming a field here is silent at compile time and
        // fails a release on the wrong side of a deploy, so the keys are pinned in one place.
        let body = serde_json::to_value(BuildInfo {
            ok: true,
            sha: "9ea50f32".to_owned(),
            version: "0.1.0 (9ea50f3)".to_owned(),
            built_at: "2026-09-28T00:00:00Z".to_owned(),
            database_target: "prod".to_owned(),
            note: None,
        })
        .expect("BuildInfo must serialize");

        assert_eq!(body["ok"], true);
        assert_eq!(body["sha"], "9ea50f32");
        assert_eq!(body["version"], "0.1.0 (9ea50f3)");
        assert_eq!(body["builtAt"], "2026-09-28T00:00:00Z");
        assert_eq!(body["databaseTarget"], "prod");
        assert!(
            body.get("note").is_none(),
            "a healthy answer stays quiet: the note is absent, not null"
        );
    }
}
