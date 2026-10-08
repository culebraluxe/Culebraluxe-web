//! RUNTIME.deploy — build stamp (TST-RUNTIME-DEPLOY-001).
//!
//! Contract: the release gate's first question — "which build is live" — is answered by the stamp the deploy
//! wrote, in precedence order `CULEBRALUXE_BUILD_SHA`, `VERCEL_GIT_COMMIT_SHA`, `GIT_COMMIT_SHA`. The first
//! candidate that IS a commit sha is served with a version labelled by its 7-char abbreviation; a candidate that
//! is set but is NOT a sha does not fall through — it is refused with the reason named, so a tag can neither
//! outrank the real stamp nor hide behind the next variable; nothing stamped means an empty sha with a note
//! saying which variable to set; and a stamp is never invented from nothing. The crate version is real either
//! way, so `version` is never empty.
//!
//! Level: L3 Composition — the production resolution the `build_info` route serves, driven through the harness
//! seam (`web::api::build_info::resolve_stamp`), no database, no network, no PROD writes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_deploy__001__build_stamp

use web::api::build_info::{resolve_stamp, MAX_SHA, MIN_SHA};

const FULL: &str = "9ea50f32b1c4d5e6f708192a3b4c5d6e7f8091a2";

fn stamped(name: &str, value: &str) -> web::api::build_info::Stamp {
    resolve_stamp(&[(name, Some(value.to_owned()))])
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-DEPLOY-001).
fn runtime_deploy_001__build_stamp() {
    // A full sha is served, labelled with its abbreviation; the answer is quiet on the happy path.
    let stamp = stamped("CULEBRALUXE_BUILD_SHA", FULL);
    assert_eq!(stamp.sha, FULL);
    assert!(
        stamp.version.contains("(9ea50f3)"),
        "the label carries the 7-char abbreviation"
    );
    assert_eq!(stamp.note, None);
    assert!(!stamp.version.is_empty());

    // Precedence: the deploy stamp outranks the platform and local fallbacks.
    let stamp = resolve_stamp(&[
        ("CULEBRALUXE_BUILD_SHA", Some("9ea50f32".to_owned())),
        (
            "VERCEL_GIT_COMMIT_SHA",
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
        ),
        (
            "GIT_COMMIT_SHA",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()),
        ),
    ]);
    assert_eq!(stamp.sha, "9ea50f32");
    // An empty first candidate does not block the next.
    let stamp = resolve_stamp(&[
        ("CULEBRALUXE_BUILD_SHA", Some("  ".to_owned())),
        ("VERCEL_GIT_COMMIT_SHA", Some("9ea50f32".to_owned())),
    ]);
    assert_eq!(stamp.sha, "9ea50f32");

    // A tag is refused with the reason named — never served, never fallen through.
    let stamp = stamped("CULEBRALUXE_BUILD_SHA", "v1.2.3");
    assert!(stamp.sha.is_empty(), "a tag is not a build");
    assert!(
        !stamp.version.is_empty(),
        "the version is real even unstamped"
    );
    let note = stamp.note.clone().unwrap_or_default();
    assert!(
        note.contains("CULEBRALUXE_BUILD_SHA"),
        "the note names the variable: {note}"
    );
    let blocked = resolve_stamp(&[
        ("CULEBRALUXE_BUILD_SHA", Some("v1.2.3".to_owned())),
        ("VERCEL_GIT_COMMIT_SHA", Some("9ea50f32".to_owned())),
    ]);
    assert!(
        blocked.sha.is_empty(),
        "a refused stamp hides nothing and outranks nothing"
    );

    // Nothing stamped: empty sha, and the note says which variable to set.
    let stamp = resolve_stamp(&[("CULEBRALUXE_BUILD_SHA", None)]);
    assert!(stamp.sha.is_empty());
    assert!(stamp
        .note
        .unwrap_or_default()
        .contains("CULEBRALUXE_BUILD_SHA"));

    // A stamp is never invented from nothing: no environment, no sha. Not "unknown", not HEAD.
    let stamp = resolve_stamp(&[]);
    assert!(stamp.sha.is_empty());

    // The bounds the predicate enforces are git's own: 7..=40.
    assert_eq!(MIN_SHA, 7);
    assert_eq!(MAX_SHA, 40);

    // Negative controls: near-shas that must never be served as a build.
    assert!(
        stamped("CULEBRALUXE_BUILD_SHA", "9ea50f").sha.is_empty(),
        "6 digits is not a sha"
    );
    assert!(
        stamped("CULEBRALUXE_BUILD_SHA", &format!("{FULL}a"))
            .sha
            .is_empty(),
        "41 digits is not a sha"
    );
    assert!(
        stamped("CULEBRALUXE_BUILD_SHA", "not-a-sha-at-all")
            .sha
            .is_empty(),
        "prose is not a sha"
    );
    assert!(
        stamped("CULEBRALUXE_BUILD_SHA", "9ea50f3!").sha.is_empty(),
        "non-hex is not a sha"
    );
}
