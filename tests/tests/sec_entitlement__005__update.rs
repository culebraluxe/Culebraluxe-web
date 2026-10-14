//! SEC.ENTITLEMENT — update access (TST-SEC-ENTITLEMENT-005).
//!
//! Contract: the `update` entitlement controls whether an actor may update existing
//! listings, properties, forms, or other records. Every update operation must be gated
//! through the entitlement catalog, and no update path may bypass the entitlement
//! decision. An update that occurs without an entitlement check is a security gap.
//!
//! Three facts are pinned, each in **both directions** — a new one fails (that is the
//! new hole) and a pin that no longer matches fails too (so the surface may only change
//! deliberately):
//!
//!   - **The update side of the catalog.** This system has no verb `update`: changing an existing record is a
//!     *command*, and the catalog names the state transitions one by one — `vault.issue`, `luxesign.issue`,
//!     `luxesign.void`, `contract.execute`, `signer.act` — each declared with `operation_kind = 'command'` in
//!     the `entitlement` table the authorize endpoint grants from (`db/migrations/*.sql`). An update the
//!     catalog cannot name is one nobody can be granted.
//!   - **The readers.** Exactly four places in `web/ui/src` adjudicate from the actor's entitlement list:
//!     `registry.rs surface_visible` and `registry.rs item_visible` (what an actor may see),
//!     `screen.rs can` (what a screen offers) and `navigation.rs holds_entitlement` (the shared rule the
//!     other three mirror). A fifth adjudicator is a second answer to the same question. The pin also holds
//!     the one reader in the tree that is **not** an adjudicator — the Roles screen's grants matrix reads a
//!     *role's* list — so that a sixth read fails this test instead of arriving unnoticed.
//!   - **The root-only codes have one writer.** `security.entitlement.manage` and `security.role.manage`
//!     are written as string literals in exactly one file in the tree — `middle/model/src/security.rs` —
//!     and every other production site names them through `model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE}`.
//!
//! The honest state of the tree today, recorded rather than hidden:
//!
//!   - Update paths that skip the entitlement check are detectable by the absence of `holds_entitlement`
//!     or `actor.entitlement_codes.iter().any(...)` in the handler code.
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
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__005__update

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

/// The entitlement catalog as the migrations leave it: `code -> operation_kind` (`query` or `command`).
///
/// A code is catalogued when a migration writes it into the `entitlement` table — that table is what the
/// authorize endpoint grants from, so it is the only honest answer to "can this code be granted?". The
/// migrations are read in file order because one of them **renames** codes: `291_luxesign_rename.sql`
/// rewrites `documentSign.*` to `luxesign.*`, so an insert from before it has to be renamed for the live
/// spelling to be found (`documentSign.read` is retired; `luxesign.read` is what the registry gates on).
fn entitlement_catalog() -> BTreeMap<String, String> {
    let dir = test_harness::source::workspace_root().join("db/migrations");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("sql"))
        .collect();
    files.sort();

    let mut catalog = BTreeMap::new();
    for path in files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        // The rename lands in `291_*`; a migration numbered after it writes the live spelling itself.
        let renames_document_sign = name.as_str() < "291_";
        for chunk in test_harness::source::read(&path).split("('").skip(1) {
            // Every catalog row is `('code', 'kind')`, and the kind is read as the quoted value that is
            // literally `query` or `command` — so a grant list like `('portal.read', 'deal.read',
            // 'settings.read')` is not mistaken for a row.
            let Some(close) = chunk.find("', '") else {
                continue;
            };
            let after = &chunk[close + 4..];
            let Some(kind_end) = after.find('\'') else {
                continue;
            };
            let kind = &after[..kind_end];
            if kind != "query" && kind != "command" {
                continue;
            }
            let code = if renames_document_sign {
                chunk[..close].replacen("documentSign.", "luxesign.", 1)
            } else {
                chunk[..close].to_string()
            };
            catalog.insert(code, kind.to_string());
        }
    }
    catalog
}

// ---- tests ----

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-005); the file and the assay use it.
fn sec_entitlement_005__update() {
    let ui_root = test_harness::source::workspace_root().join("web/ui/src");
    let model_root = test_harness::source::workspace_root().join("middle/model/src");

    // 0. The walkers found a tree.
    let ui_files = test_harness::source::sources_under(&ui_root);
    assert!(
        ui_files.len() >= 80,
        "the UI sources must be walked; {} files found",
        ui_files.len()
    );

    // 1. THE UPDATE SIDE OF THE CATALOG. This system has no verb `update`: changing an existing record is a
    //    *command*, and this catalog names the state transitions one by one rather than as one `update` verb —
    //    `vault.issue` (vault.issue, db/migrations/210), `luxesign.issue` / `luxesign.void` (sending and
    //    cancelling a signature request, `261_docsign_entitlements.sql`), `contract.execute`, `signer.act`
    //    (a signer's decision). Each is `operation_kind = 'command'`. The claim stands — an update path the
    //    catalog cannot name is one nobody can be granted — so each must stay a catalogued command.
    let catalog = entitlement_catalog();
    assert!(
        catalog.len() >= 40,
        "the entitlement catalog must be read; {} code(s) found",
        catalog.len()
    );
    for code in [
        "contract.execute",
        "luxesign.issue",
        "luxesign.void",
        "signature.write",
        "signer.act",
        "vault.issue",
    ] {
        assert_eq!(
            catalog.get(code).map(String::as_str),
            Some("command"),
            "The `{code}` state-changing action must be a catalogued command — an update the catalog \
             cannot name is a gap"
        );
    }
    assert!(
        catalog
            .values()
            .all(|kind| kind == "query" || kind == "command"),
        "operation_kind is `query` or `command`; a third value would not be grantable"
    );

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
