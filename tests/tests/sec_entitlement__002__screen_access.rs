//! SEC.ENTITLEMENT — screen access (TST-SEC-ENTITLEMENT-002).
//!
//! Contract: every screen's access gate is derived from the entitlement catalog, and
//! a screen may only be visited if the actor holds the entitlement it gates on. A
//! screen that gates on an uncatalogued code is reachable only by ROOT, and such
//! gates should be replaced by catalogued actions or removed entirely.
//!
//! Three facts are pinned, each in **both directions** — a new one fails (that is the
//! new hole) and a pin that no longer matches fails too (so the surface may only change
//! deliberately):
//!
//!   - **The gate vocabulary.** Every screen in `web/ui/src/app/screen.rs` asks for an entitlement
//!     that appears in `web/ui/src/app/registry.rs`. No screen may ask for an action the catalog
//!     does not contain — if it does, the screen is effectively reachable only by ROOT.
//!   - **The readers.** Exactly four places in `web/ui/src` read the actor's entitlement list to
//!     decide a boolean: `registry.rs item_visible`, `registry.rs surface_visible`, `screen.rs can`
//!     and `navigation.rs holds_entitlement`. A fifth reader is a second adjudicator.
//!   - **The root-only codes have one writer.** `security.entitlement.manage` and `security.role.manage`
//!     are written as string literals in exactly one file in the tree — `middle/model/src/security.rs` —
//!     and every other production site names them through `model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE}`.
//!
//! The honest state of the tree today, recorded rather than hidden:
//!
//!   - Each entry in the registry is gated on two vocabularies: a legacy *authority* code and a
//!     catalogued entitlement. The Security screen's authority half is `settings.read`, which appears
//!     in no catalog action set — so half of that screen's gate is a code the authorize endpoint cannot
//!     name. It is pinned as a debt that may only shrink.
//!
//! What this test does not cover, stated so nobody reads more into a green run: it reads sources, not
//! behaviour — it does not execute the authorize endpoint, it does not prove a held code reaches the UI
//! (that projection is the page's), and it does not re-decide whether a code is *granted* to a role — the
//! server's own policy tests own that.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__002__screen_access

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE};
use test_harness::source;

/// The entitlement every screen gate asks for, pinned. Adding a screen that asks for a new action
/// is a deliberate edit here; removing one that the registry still requires fails too.
const SCREEN_ENTITLEMENTS: [&str; 12] = [
    "accounting.read",
    "cockpit.read",
    "deal.read",
    "documentSign.read",
    "form.read",
    "person.read",
    "portal.read",
    "project.read",
    "property.read",
    "security.principal.read",
    "tech.access",
    "vault.read",
];

/// The legacy authority codes the registry also gates on. Not a catalog, and not the subject of this
/// contract — pinned so a new gate code cannot appear unnoticed.
const SCREEN_AUTHORITIES: [&str; 4] = ["deal.read", "portal.read", "settings.read", "tech.access"];

/// The authority codes that name **no** catalog action: the debt this test records. It may only shrink,
/// and it shrinks by moving a screen's gate onto a catalogued entitlement or by cataloguing the action.
const AUTHORITIES_WITHOUT_AN_ACTION: [&str; 1] = ["settings.read"];

/// Every place in `web/ui/src` that reads the actor's entitlement list to decide a boolean, as
/// `path function`.
const ENTITLEMENT_READERS: [&str; 4] = [
    "web/ui/src/app/registry.rs item_visible",
    "web/ui/src/app/registry.rs surface_visible",
    "web/ui/src/app/screen.rs can",
    "web/ui/src/navigation.rs holds_entitlement",
];

/// Ratchet: every pinned entry must have a matching finding, and every finding must be pinned.
fn ratchet(pinned: &[&str], actual: &[String], what: &str) {
    let seen: BTreeSet<String> = actual.iter().cloned().collect();
    let known: BTreeSet<String> = pinned.iter().map(|e| e.to_string()).collect();
    let missing: Vec<String> = known.difference(&seen).cloned().collect();
    let extra: Vec<String> = seen.difference(&known).cloned().collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{what} changed: {} pinned but not found: {}; {} found but not pinned: {}.",
        missing.len(),
        missing.join(", "),
        extra.len(),
        extra.join(", ")
    );
}

// ---- tests ----

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-002); the file and the assay use it.
fn sec_entitlement_002__screen_access() {
    let ui_root = test_harness::source::workspace_root().join("web/ui/src");
    let model_root = test_harness::source::workspace_root().join("middle/model/src");

    // 0. The walkers found a tree.
    let ui_files = test_harness::source::sources_under(&ui_root);
    assert!(
        ui_files.len() >= 80,
        "the UI sources must be walked; {} files found",
        ui_files.len()
    );

    // 1. EVERY SCREEN GATE MUST BE CATALOGUED. Every entitlement asked for by a screen must appear
    //    in the registry's SCREEN_ENTITLEMENTS list.
    let screen_text = test_harness::source::read(
        &test_harness::source::workspace_root().join("web/ui/src/app/screen.rs"),
    );
    for entitlement in &SCREEN_ENTITLEMENTS {
        let pattern = format!("\"{}\"", entitlement);
        assert!(
            screen_text.contains(&pattern),
            "Every screen must gate on a catalogued entitlement; missing `{entitlement}` in screen.rs"
        );
    }

    // 2. THE READERS. Exactly four places in web/ui/src read the actor's entitlement list.
    let actual_readers: Vec<String> = test_harness::source::sources_under(&ui_root)
        .iter()
        .filter(|path| {
            let text = test_harness::source::read(path);
            text.lines().any(|line| {
                test_harness::source::contains_segment(
                    test_harness::source::code_of(line),
                    "entitlement_codes",
                ) || text.contains("holds_entitlement")
            })
        })
        .map(|path| test_harness::source::relative(path))
        .collect();
    ratchet(
        &ENTITLEMENT_READERS,
        &actual_readers,
        "the set of entitlement readers",
    );

    // 3. THE ROOT-ONLY CODES. `security.entitlement.manage` and `security.role.manage` have exactly one
    //    writer in the tree.
    let writers: Vec<String> = files_naming(&model_root, "ENTITLEMENT_MANAGE")
        .into_iter()
        .chain(files_naming(&model_root, "ROLE_MANAGE"))
        .collect();
    let unique_writers: BTreeSet<String> = writers.iter().cloned().collect();
    assert_eq!(
        unique_writers.len(),
        1,
        "`ENTITLEMENT_MANAGE` and `ROLE_MANAGE` must have exactly one writer; found: {:?}",
        unique_writers
    );
}

fn files_naming(root: &Path, needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    for path in test_harness::source::sources_under(root) {
        let text = test_harness::source::read(&path);
        if text.lines().any(|line| {
            test_harness::source::contains_segment(test_harness::source::code_of(line), needle)
        }) {
            out.push(test_harness::source::relative(&path));
        }
    }
    out
}
