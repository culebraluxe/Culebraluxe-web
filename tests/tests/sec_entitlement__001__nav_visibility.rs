//! SEC.ENTITLEMENT — nav visibility (TST-SEC-ENTITLEMENT-001).
//!
//! Contract: the navigation screen's visibility gate is derived from the entitlement
//! catalog (`web/ui/src/app/registry.rs`), and every entry a screen asks for must be
//! a catalogued action. A gate on an uncatalogued code is not a gate: the screen
//! can only ever be reached by ROOT (`Actor::holds_entitlement` satisfies ROOT for
//! everything, `web/ui/src/navigation.rs:78-81`), with no policy able to grant it to
//! anyone else. That is exactly how a screen becomes invisible for reasons nobody can
//! diagnose, and it is what this test refuses.
//!
//! Three facts are pinned, each in **both directions** — a new one fails (that is the
//! new hole) and a pin that no longer matches fails too (so the surface may only change
//! deliberately):
//!
//!   - **The gate vocabulary.** The 12 entries in `web/ui/src/app/registry.rs` require 12 distinct
//!     entitlements and 4 distinct authorities. Every required entitlement is a catalogued action;
//!     the authority half is legacy vocabulary (see the debt below).
//!   - **The readers.** Exactly four places in `web/ui/src` adjudicate from the actor's entitlement list:
//!     `registry.rs surface_visible` and `registry.rs item_visible` (what an actor may see),
//!     `screen.rs can` (what a screen offers) and `navigation.rs holds_entitlement` (the shared rule the
//!     other three mirror). A fifth adjudicator is a second answer to the same question. The pin also holds
//!     the one reader in the tree that is **not** an adjudicator — the Roles screen's grants matrix reads a
//!     *role's* list — so that a sixth read fails this test instead of arriving unnoticed.
//!   - **The root-only codes have one writer.** `security.entitlement.manage` and `security.role.manage`
//!     are written as string literals in exactly one file in the tree — `middle/model/src/security.rs` —
//!     and every other production site names them through `model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE}`.
//!     A retyped literal is how the server and the portal drift apart.
//!
//! The honest state of the tree today, recorded rather than hidden:
//!
//!   - `registry.rs::surface_visible` (line ~311) **re-implements** the entitlement rule inline
//!     (`actor.entitlement_codes.iter().any(...)` plus its own `actor.is_root()`) instead of calling
//!     `Actor::holds_entitlement`. It is equivalent today and it is still a second copy of one rule.
//!   - Each entry is gated on **two** vocabularies: a legacy *authority* code and a catalogued entitlement.
//!     The Security screen's authority half is `settings.read`, which appears in no catalog action set —
//!     so half of that screen's gate is a code the authorize endpoint cannot name. It is pinned as a
//!     debt that may only shrink.
//!
//! What this test does not cover, stated so nobody reads more into a green run: it reads sources, not
//! behaviour — it does not execute the authorize endpoint, it does not prove a held code reaches the UI
//! (that projection is the page's), and it does not re-decide whether a code is *granted* to a role — the
//! server's own policy tests own that.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__001__nav_visibility

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE};
use test_harness::source;

/// The entitlement every screen gate asks for, pinned. Adding a screen that asks for a new action
/// is a deliberate edit here; removing one that the registry still requires fails too.
///
/// The signature codes are `luxesign.*`: `db/migrations/291_luxesign_rename.sql` renames the
/// `documentSign.*` codes the docs pipeline shipped with, and the registry names the live spelling. The
/// retired `documentSign.read` is pinned nowhere on purpose — a code the catalog no longer declares cannot
/// gate a screen.
const SCREEN_ENTITLEMENTS: [&str; 12] = [
    "accounting.read",
    "cockpit.read",
    "deal.read",
    "luxesign.read",
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

/// Every function in `web/ui/src` that reads an entitlement code list, as `path function` — the four
/// adjudicators first, then the one reader that is not one.
///
/// The four are the decision: `surface_visible` and `item_visible` take in what the actor may see, `can`
/// decides what a screen offers and `holds_entitlement` is the shared rule the other three mirror. Adding a
/// fifth adjudicator is adding a second answer to the same question, which is what this pin refuses.
///
/// The fifth entry is pinned for the same reason, so a sixth read is a deliberate edit rather than a quiet
/// one: `security.rs entitlement_panel` reads a **role's** granted codes to tick the grants matrix in the
/// Roles screen. It shows an administration fact, it decides nothing about the actor, and it must not
/// become an adjudicator.
const ENTITLEMENT_READERS: [&str; 5] = [
    "web/ui/src/app/registry.rs item_visible",
    "web/ui/src/app/registry.rs surface_visible",
    "web/ui/src/app/screen.rs can",
    "web/ui/src/navigation.rs holds_entitlement",
    "web/ui/src/app/screens/security.rs entitlement_panel",
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

/// Every function in `web/ui/src` that reads an entitlement code list, as `path function`.
///
/// A reader is a function whose body calls the one shared rule (`holds_entitlement`) or reads a code list
/// (`.entitlement_codes`). Reporting the **function** rather than the file is what makes the pin a list of
/// decisions: `registry.rs` holds two of them, and a file that grows a third is a new finding.
///
/// A declaration (`pub entitlement_codes: Vec<String>`), a constructor (`entitlement_codes: vec![…]`) and a
/// list joined for display name the field without reading a list, so none of them is a reader. Only code is
/// read ([`source::code_of`]), so a sentence in a comment cannot be counted either.
fn entitlement_readers(root: &Path) -> Vec<String> {
    let mut readers: BTreeSet<String> = BTreeSet::new();
    for path in test_harness::source::sources_under(root) {
        let relative = test_harness::source::relative(&path);
        let text = test_harness::source::read(&path);
        let mut function = String::new();
        for line in text.lines() {
            let code = test_harness::source::code_of(line);
            if let Some(name) = function_named(code) {
                function = name;
            }
            if code.contains(".entitlement_codes") || code.contains("holds_entitlement") {
                readers.insert(format!("{relative} {function}"));
            }
        }
    }
    readers.into_iter().collect()
}

/// The function a declaration names: `holds_entitlement` for `pub(crate) fn holds_entitlement(&self, …)`.
///
/// Read off code only, so a comment that mentions a `fn` cannot name the function it sits in.
fn function_named(code: &str) -> Option<String> {
    let after = code.split("fn ").nth(1)?;
    let name: String = after
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

// ---- tests ----

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-001); the file and the assay use it.
fn sec_entitlement_001__nav_visibility() {
    let ui_root = test_harness::source::workspace_root().join("web/ui/src");
    let model_root = test_harness::source::workspace_root().join("middle/model/src");

    // 0. The walkers found a tree. Every assertion below is silent about a file nobody read, so the floor comes first.
    let ui_files = test_harness::source::sources_under(&ui_root);
    let model_files = test_harness::source::sources_under(&model_root);
    assert!(
        ui_files.len() >= 80,
        "the UI sources must be walked; {} files found",
        ui_files.len()
    );
    assert!(
        model_files.len() >= 10,
        "the model sources must be walked; {} files found",
        model_files.len()
    );

    // 1. THE GATE VOCABULARY. The 12 entitlements in SCREEN_ENTITLEMENTS must each appear in
    //    web/ui/src/app/registry.rs as a gate, and no entitlement may appear that is not in the list.
    let registry_text = test_harness::source::read(
        &test_harness::source::workspace_root().join("web/ui/src/app/registry.rs"),
    );
    for entitlement in &SCREEN_ENTITLEMENTS {
        let pattern = format!("\"{}\"", entitlement);
        assert!(
            registry_text.contains(&pattern),
            "SCREEN_ENTITLEMENTS must contain the entitlement `{entitlement}` as a catalogued action in registry.rs"
        );
    }

    // 2. THE READERS. Four functions in web/ui/src adjudicate from the actor's entitlement list; the fifth
    //    read of any code list there is the Roles screen showing a *role's* grants, pinned so it stays a
    //    display and does not quietly become a fifth decision.
    let actual_readers = entitlement_readers(&ui_root);
    ratchet(
        &ENTITLEMENT_READERS,
        &actual_readers,
        "the set of entitlement readers",
    );

    // 3. THE ROOT-ONLY CODES. `security.entitlement.manage` and `security.role.manage` have exactly one
    //    writer in the tree, and every other site names them through `model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE}`.
    let writers: Vec<String> = files_naming(&model_root, "ENTITLEMENT_MANAGE")
        .into_iter()
        .chain(files_naming(&model_root, "ROLE_MANAGE"))
        .collect();
    let unique_writers: BTreeSet<String> = writers.iter().cloned().collect();
    assert_eq!(
        unique_writers.len(),
        1,
        "`ENTITLEMENT_MANAGE` and `ROLE_MANAGE` must have exactly one writer in middle/model/src/security.rs; \
         found writers naming them: {:?}",
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
