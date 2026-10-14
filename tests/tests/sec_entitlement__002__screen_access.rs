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
//!   - **The gate vocabulary.** Every screen's `entry(...)` line in `web/ui/src/app/registry.rs` names an
//!     authority half and an entitlement half, and each half is pinned here in both directions. The
//!     entitlement half must be declared in the `entitlement` catalog the authorize endpoint grants from
//!     (`db/migrations/*.sql`) — except the recorded debt below. A gate on a code the catalog cannot name
//!     is reachable only by ROOT, which is the hole this contract exists to catch.
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
///
/// `tech.access` is the second one, and it is the sharper half of the debt: it is a *setting* in
/// `015_auth_security_model.sql` and an authority in `083_role_tech_entitlement.sql`, never a row of the
/// `entitlement` table — so the entitlement the three TECH screens gate on cannot be granted to any role.
///
/// [`GATES_WITHOUT_AN_ACTION`] records `tech.access` on the entitlement half, where it makes three screens
/// invisible to everyone but ROOT.
const AUTHORITIES_WITHOUT_AN_ACTION: [&str; 2] = ["settings.read", "tech.access"];

/// The screen **entitlement** halves that name no catalog action: the debt this case records, and it may
/// only shrink.
///
/// This is the entitlement half of [`AUTHORITIES_WITHOUT_AN_ACTION`], minus `settings.read`, which is only
/// ever an authority. `tech.access` is a setting (`015_auth_security_model.sql`) and an authority
/// (`083_role_tech_entitlement.sql`), never a row of the `entitlement` table — so the three TECH screens
/// (Engineering, Storyboard, UI Lab) gate on a code no role can be granted and only ROOT reaches them. The
/// repair is to catalogue the action or to move the gate onto a catalogued one; deleting this pin without
/// doing either is how a hole becomes invisible again.
const GATES_WITHOUT_AN_ACTION: [&str; 1] = ["tech.access"];

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

/// The gate each screen in `web/ui/src/app/registry.rs` asks for: `(authority, entitlement)`, either half of
/// which may be empty for a screen nobody has to hold anything to reach.
///
/// The entry signature puts the gate last, immediately before the kind — `entry("deals", "/portal/deals",
/// Surface::Core, "Contracts", Menu::Rail("Contracts"), "deal.read", "deal.read", Kind::Screen(..))` — so
/// the gate is the last two quoted arguments *before* `Kind::`, read positionally rather than by shape:
/// `""` is a real half (19 site and auth screens carry no gate at all, and `portal-root` carries only an
/// authority), and the `Kind::` bound keeps a trailing `.of("marketing")` out of the read. The table is
/// `#[rustfmt::skip]`ped with one entry per line, so a line is a whole entry and a line whose gate cannot be
/// read is a failure rather than a skip.
fn registry_gates(registry_text: &str) -> Vec<(String, String)> {
    let mut gates = Vec::new();
    for line in registry_text
        .lines()
        .filter(|line| line.trim_start().starts_with("entry(\""))
    {
        let before_kind = line.split("Kind::").next().unwrap_or_default();
        let halves: Vec<&str> = before_kind.split('"').skip(1).step_by(2).collect();
        match halves.as_slice() {
            [.., authority, entitlement] => {
                gates.push((authority.to_string(), entitlement.to_string()));
            }
            _ => panic!(
                "every entry line names an authority half and an entitlement half before its kind: {line}"
            ),
        }
    }
    gates
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

    // 1. EVERY SCREEN GATE MUST BE CATALOGUED — against the catalog that authorizes, not against
    //    `screen.rs`. `screen.rs`'s `can()` reads the actor's code list without naming one, so asking it for
    //    the 12 codes asked a file that never carried them, and the case could not fail for the reason it
    //    meant. The vocabulary lives in two places: the registry's `entry(...)` lines (what each screen asks
    //    for) and the `entitlement` table the authorize endpoint grants from (what can be granted).
    let registry_text = test_harness::source::read(
        &test_harness::source::workspace_root().join("web/ui/src/app/registry.rs"),
    );
    let gates = registry_gates(&registry_text);
    assert!(
        gates.len() >= 20,
        "the registry's entries must be read; {} gate(s) found",
        gates.len()
    );

    // 1a. The entitlement half: every code pinned in SCREEN_ENTITLEMENTS is still asked for by a screen, and
    //     every asked-for code is declared in the catalog — except the recorded debt, which may only shrink.
    //     An empty half is a screen with no entitlement gate (19 screens, all of them public or auth pages).
    let asked_entitlements: BTreeSet<String> = gates
        .iter()
        .map(|(_, entitlement)| entitlement.clone())
        .filter(|code| !code.is_empty())
        .collect();
    let pinned_entitlements: BTreeSet<String> =
        SCREEN_ENTITLEMENTS.iter().map(|code| code.to_string()).collect();
    assert_eq!(
        asked_entitlements, pinned_entitlements,
        "the entitlements the screens gate on changed; pin the new one deliberately or remove it"
    );

    let catalog = entitlement_catalog();
    assert!(
        catalog.len() >= 40,
        "the entitlement catalog must be read; {} code(s) found",
        catalog.len()
    );
    let uncatalogued: BTreeSet<String> = asked_entitlements
        .iter()
        .filter(|code| !catalog.contains_key(*code))
        .cloned()
        .collect();
    let gate_debt: BTreeSet<String> =
        GATES_WITHOUT_AN_ACTION.iter().map(|code| code.to_string()).collect();
    assert_eq!(
        uncatalogued, gate_debt,
        "the entitlement gates that name no catalogued action changed: {} now uncatalogued, {} recorded. \
         A new one makes a screen ROOT-only; a recorded one that is now catalogued shrinks the debt here.",
        uncatalogued.len(),
        gate_debt.len()
    );

    // 1b. The authority half: legacy vocabulary, not a catalog, pinned in both directions so a new gate code
    //     cannot appear unnoticed — with its own recorded debt.
    let asked_authorities: BTreeSet<String> = gates
        .iter()
        .map(|(authority, _)| authority.clone())
        .filter(|code| !code.is_empty())
        .collect();
    let pinned_authorities: BTreeSet<String> =
        SCREEN_AUTHORITIES.iter().map(|code| code.to_string()).collect();
    assert_eq!(
        asked_authorities, pinned_authorities,
        "the authorities the screens gate on changed; pin the new one deliberately or remove it"
    );
    let authorities_without_an_action: BTreeSet<String> = asked_authorities
        .iter()
        .filter(|code| !catalog.contains_key(*code))
        .cloned()
        .collect();
    let authority_debt: BTreeSet<String> = AUTHORITIES_WITHOUT_AN_ACTION
        .iter()
        .map(|code| code.to_string())
        .collect();
    assert_eq!(
        authorities_without_an_action, authority_debt,
        "the authority gates that name no catalogued action changed: {} now uncatalogued, {} recorded. \
         The authority half is legacy naming, but a third code with no action would be a third gate that \
         cannot be granted.",
        authorities_without_an_action.len(),
        authority_debt.len()
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
