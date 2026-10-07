//! RUNTIME.deploy — commit SHA (TST-RUNTIME-DEPLOY-002).
//!
//! Contract: a commit sha is 7..=40 lowercase hex digits and nothing else — the shortest and longest
//! abbreviations git itself produces. The stamp is trimmed (a `git rev-parse` substitution carries a newline;
//! an env file may keep padding), normalized to lowercase so the release compare is exact rather than
//! case-dependent, and anything else — tags, prose, wrong lengths, non-hex — is refused instead of served.
//!
//! Level: L3 Composition — the production sha predicate behind the `build_info` stamp, driven through the
//! harness seam (`web::api::build_info`), no database, no network, no PROD writes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_deploy__002__commit_sha

use web::api::build_info::{resolve_stamp, MAX_SHA, MIN_SHA};

/// The sha the production resolution serves for a single stamped candidate (empty when refused).
fn served(candidate: &str) -> String {
    resolve_stamp(&[("CULEBRALUXE_BUILD_SHA", Some(candidate.to_owned()))]).sha
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-DEPLOY-002).
fn runtime_deploy_002__commit_sha() {
    // The bounds are git's own abbreviations, pinned as constants.
    assert_eq!(
        MIN_SHA, 7,
        "shortest abbreviation git rev-parse --short returns"
    );
    assert_eq!(MAX_SHA, 40, "longest abbreviation git itself produces");
    assert_eq!(served("9ea50f3"), "9ea50f3", "the 7-digit floor is a sha");
    assert_eq!(
        served("9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2"),
        "9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2",
        "the full 40-digit sha is served whole"
    );

    // Trimmed: command substitution trails a newline, env files keep padding — neither may void a real stamp.
    assert_eq!(served("9ea50f32\n"), "9ea50f32");
    assert_eq!(served("  9ea50f32  "), "9ea50f32");

    // Normalised: git abbreviations are already lowercase, so serving lower makes the smoke compare exact.
    assert_eq!(served("9EA50F3"), "9ea50f3");
    assert_eq!(
        served("9EA50F32B1C4D5E6F708192A3B4C5D6E7F8091A2").to_ascii_lowercase(),
        served("9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2")
    );

    // Refused: tags, prose, wrong lengths, non-hex — a release check must fail loudly, not pass quietly.
    for bad in [
        "v1.2.3",
        "latest",
        "9ea50f",
        "9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2a",
        "9ea50f3!",
        "zzzzzzz",
        "9ea50f3\nv1.2.3",
    ] {
        assert!(
            served(bad).is_empty(),
            "{bad:?} is not a commit sha and must be refused"
        );
    }
    // The two whitespace truths, stated exactly: blank is unset (empty sha), padded is trimmed (served).
    assert!(served("").is_empty());
    assert!(served("   ").is_empty());
    assert_eq!(served("9ea50f3 "), "9ea50f3");

    // The served label uses the normalized prefix: version names the abbreviated commit.
    let stamp = resolve_stamp(&[("CULEBRALUXE_BUILD_SHA", Some("9EA50F32".to_owned()))]);
    assert_eq!(stamp.sha, "9ea50f32");
    assert!(
        stamp.version.contains("(9ea50f3)"),
        "the label uses the served form: {}",
        stamp.version
    );
}
