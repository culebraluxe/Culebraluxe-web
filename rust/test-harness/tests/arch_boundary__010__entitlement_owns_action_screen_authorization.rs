//! ARCH.BOUNDARY — Entitlement owns action/screen authorization (TST-ARCH-BOUNDARY-010).
//!
//! Contract: a screen's visibility and an offered control are **one entitlement decision each**, made from a vocabulary
//! the server can actually decide. `web/src/security/entitlement_catalog.rs` is that vocabulary — an action it
//! cannot name cannot be decided by the authorize endpoint, and the file says so — so **no screen may gate itself on a
//! code the catalog does not contain**. A gate on an uncatalogued code is not a gate: it is a screen that can only ever
//! be reached by ROOT (`Actor::holds_entitlement` satisfies ROOT for everything, `web/ui/src/navigation.rs:78-81`),
//! with no policy able to grant it to anyone else. That is exactly how a screen becomes invisible for reasons nobody can
//! diagnose, and it is what this test refuses.
//!
//! Three facts are pinned, each in **both directions** — a new one fails (that is the new hole) and a pin that no longer
//! matches fails too (so the surface may only change deliberately):
//!
//!   - **The gate vocabulary.** The 60 entries in `web/ui/src/app/registry.rs` require 12 distinct entitlements and 4
//!     distinct authorities. Every required entitlement is a catalogued action; the authority half is legacy vocabulary
//!     (see the debt below).
//!   - **The readers.** Exactly four places in `web/ui/src` read the actor's entitlement list to decide a boolean:
//!     `registry.rs item_visible`, `registry.rs surface_visible`, `screen.rs can` and `navigation.rs holds_entitlement`.
//!     A fifth reader is a second adjudicator.
//!   - **The root-only codes have one writer.** `security.entitlement.manage` and `security.role.manage` are written as
//!     string literals in exactly one file in the tree — `middle/model/src/security.rs` — and every other production
//!     site names them through `model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE}`. A retyped literal is how the
//!     server and the portal drift apart.
//!
//! The honest state of the tree today, recorded rather than hidden:
//!
//!   - `registry.rs::surface_visible` (line ~311) **re-implements** the entitlement rule inline
//!     (`actor.entitlement_codes.iter().any(...)` plus its own `actor.is_root()`) instead of calling
//!     `Actor::holds_entitlement`. It is equivalent today and it is still a second copy of one rule.
//!   - Each entry is gated on **two** vocabularies: a legacy *authority* code and a catalogued entitlement. The Security
//!     screen's authority half is `settings.read`, which appears in no catalog action set — so half of that screen's
//!     gate is a code the authorize endpoint cannot name. It is pinned as a debt that may only shrink.
//!
//! What this test does not cover, stated so nobody reads more into a green run: it reads sources, not behaviour. It does
//! not execute the authorize endpoint, it does not prove a held code reaches the UI (that projection is the page's), and
//! it does not re-decide whether a code is *granted* to a role — the server's own policy tests own that.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__010__entitlement_owns_action_screen_authorization

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use model::security::{ENTITLEMENT_MANAGE, ROLE_MANAGE};
use test_harness::source;

/// The entitlement every screen gate asks for, pinned. Adding a screen that asks for a new action is a deliberate edit
/// here; removing one that the registry still requires fails too.
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

/// The legacy authority codes the registry also gates on. Not a catalog, and not the subject of this contract — pinned
/// so a new gate code cannot appear unnoticed.
const SCREEN_AUTHORITIES: [&str; 4] = ["deal.read", "portal.read", "settings.read", "tech.access"];

/// The authority codes that name **no** catalog action: the debt this test records. It may only shrink, and it shrinks
/// by moving a screen's gate onto a catalogued entitlement or by cataloguing the action.
const AUTHORITIES_WITHOUT_AN_ACTION: [&str; 1] = ["settings.read"];

/// Every place in `web/ui/src` that reads the actor's entitlement list to decide a boolean, as `path function`.
const ENTITLEMENT_READERS: [&str; 4] = [
    "web/ui/src/app/registry.rs item_visible",
    "web/ui/src/app/registry.rs surface_visible",
    "web/ui/src/app/screen.rs can",
    "web/ui/src/navigation.rs holds_entitlement",
];

/// The public read actions, which the anonymous door uses and which must therefore be catalogued like any other.
const PUBLIC_READ_ACTIONS: [&str; 2] = ["guide.public.read", "property.public.read"];

/// The registry's entries. A new screen is one line here; the count moves with it, deliberately.
const ENTRY_COUNT: usize = 60;

/// The entries that require no entitlement at all (a public page, a record route behind its parent's gate). Pinned so a
/// screen that silently loses its gate is visible.
const UNGATED_ENTRIES: usize = 31;

/// A floor on the catalog: a scanner that finds nothing must fail instead of reporting a clean tree. The catalog holds
/// 42 actions today and is allowed to grow.
const CATALOG_FLOOR: usize = 40;

/// A floor on the UI sources the reader scan walks. 146 `.rs` files live under `web/ui/src` today.
const UI_SOURCE_FLOOR: usize = 100;

fn in_repo(relative: &str) -> PathBuf {
    source::repo_root().join(relative)
}

/// Every double-quoted literal in `text`, in order. These action codes carry no escapes, so a literal is `"` … `"`.
fn literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

/// The arguments of a call whose opening parenthesis has already been consumed, split on top-level commas.
///
/// Stops at the parenthesis that closes the call, so a trailing `.of("parent")` is not read as an argument. A nested
/// call (`Kind::Screen(mount::<Cockpit>)`) and a literal containing a comma both survive.
fn call_args(body: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut chars = body.chars();
    while let Some(character) = chars.next() {
        if in_string {
            current.push(character);
            if character == '\\' {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                current.push(character);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(character);
            }
            ')' | ']' | '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                current.push(character);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

/// The action → operation-kind pairs the server's catalog declares, with the two `model::security` constants resolved.
///
/// A constant the test does not know is a failure, not a skip: an unnamed entry would silently leave the catalog looking
/// smaller than it is, which is the one thing this scanner must not do.
fn entitlement_catalog() -> Vec<(String, String)> {
    let text = source::read(&in_repo("web/src/security/entitlement_catalog.rs"));
    let mut out = Vec::new();
    let mut inside = false;
    let mut constants = 0;
    for line in text.lines() {
        let code = source::code_of(line).trim();
        if code.contains("const ACTIONS") {
            inside = true;
            continue;
        }
        if inside && code.starts_with("];") {
            inside = false;
            continue;
        }
        if !inside || !code.starts_with('(') {
            continue;
        }
        let body = code.trim_start_matches('(');
        if body.starts_with('"') {
            let found = literals(body);
            assert_eq!(
                found.len(),
                2,
                "web/src/security/entitlement_catalog.rs — `{code}` is not an action and its operation kind; a \
                 differently shaped entry must update this scanner, not slip past it"
            );
            out.push((found[0].clone(), found[1].clone()));
            continue;
        }
        constants += 1;
        let name = body.split(',').next().unwrap_or(body).trim();
        let action = match name {
            "model::security::ENTITLEMENT_MANAGE" => model::security::ENTITLEMENT_MANAGE,
            "model::security::ROLE_MANAGE" => model::security::ROLE_MANAGE,
            other => panic!(
                "the catalog names `{other}`, which this contract does not know: a new constant in the catalog is a \
                 deliberate addition — resolve it here so its action stays visible to the scanner"
            ),
        };
        let kind = literals(body);
        assert_eq!(kind.len(), 1, "`{name}` needs its operation kind");
        out.push((action.to_string(), kind[0].clone()));
    }
    assert_eq!(
        constants, 2,
        "the catalog names its two root-only actions through `model::security`; another constant reference is a new \
         root-only action, and a privilege change this test exists to see"
    );
    out
}

/// The public read actions, as the server's authorize seam declares them.
fn public_read_actions() -> BTreeSet<String> {
    let text = source::read(&in_repo("web/src/security/entitlements.rs"));
    let line = text
        .lines()
        .find(|line| source::code_of(line).contains("const PUBLIC_READ_ACTIONS"))
        .expect("web/src/security/entitlements.rs declares PUBLIC_READ_ACTIONS");
    literals(source::code_of(line)).into_iter().collect()
}

/// One registry entry's gate: the authority code and the entitlement code it requires.
struct Gate {
    authority: String,
    entitlement: String,
}

/// The `entry(...)` gates in `web/ui/src/app/registry.rs`.
///
/// Every call must read as the signature's eight arguments: a gate written in a shape this scanner cannot read would be
/// a gate that is not checked, so a mismatch is a failure with the reason, never a skip.
fn registry_gates() -> Vec<Gate> {
    let text = source::read(&in_repo("web/ui/src/app/registry.rs"));
    let mut out = Vec::new();
    for line in text.lines() {
        let code = source::code_of(line);
        let Some(position) = code.find("entry(") else {
            continue;
        };
        if code[..position].contains("fn ") {
            continue; // the `const fn entry(...)` definition, not an entry
        }
        let args = call_args(&code[position + "entry(".len()..]);
        assert_eq!(
            args.len(),
            8,
            "web/ui/src/app/registry.rs — this `entry(...)` call does not read as eight arguments ({} found: {:?}); the \
             registry's gate codes must stay visible to this contract",
            args.len(),
            args
        );
        let unquote = |argument: &str, what: &str| -> String {
            assert!(
                argument.starts_with('"') && argument.ends_with('"') && argument.len() >= 2,
                "web/ui/src/app/registry.rs — a gate's {what} is `{argument}`, not a string literal; a computed gate \
                 cannot be checked against the catalog"
            );
            argument.trim_matches('"').to_string()
        };
        out.push(Gate {
            authority: unquote(&args[5], "authority"),
            entitlement: unquote(&args[6], "entitlement"),
        });
    }
    out
}

/// The line that decides access **from** an entitlement list, among the line at `index` and the three after it.
///
/// A window rather than a line, so the multi-line `actor` / `.entitlement_codes` / `.iter()` / `.any(..)` chain is read
/// as one decision; the *deciding* line is returned, not the first line of the window, so the reader is attributed to
/// the function that makes the decision and not to the blank line above it. Two shapes count: a call to
/// `holds_entitlement(..)` — the one rule — and a test of the list itself (`.any(..)` after it is named). A declaration
/// (`pub entitlement_codes: Vec<String>,`), a wire parse (`entitlement_codes: strings("entitlementCodes")`) and a
/// fixture construction (`.iter().map(..).collect()`) name the list but decide nothing, and a definition line never
/// decides: a `fn` names the rule.
fn decision_line(lines: &[&str], index: usize) -> Option<usize> {
    let end = (index + 4).min(lines.len());
    let mut saw_entitlements = false;
    for offset in 0..end - index {
        let code = source::code_of(lines[index + offset]);
        if code.contains("fn ") {
            continue;
        }
        if code.contains("holds_entitlement(") {
            return Some(index + offset);
        }
        if code.contains("entitlement_codes") {
            saw_entitlements = true;
        }
        if saw_entitlements && code.contains(".any(") {
            return Some(index + offset);
        }
    }
    None
}

/// The name of the function a one-based line sits inside, or `<none>` when nothing encloses it.
fn enclosing_fn(text: &str, first_line: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    for index in (0..first_line.saturating_sub(1).min(lines.len())).rev() {
        let code = source::code_of(lines[index]);
        let Some(position) = code.find("fn ") else {
            continue;
        };
        let rest = &code[position + 3..];
        let name: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() && matches!(rest[name.len()..].chars().next(), Some('(' | '<')) {
            return name;
        }
    }
    "<none>".to_string()
}

/// Every place under `web/ui/src` that decides from an entitlement list, as `path function`.
fn entitlement_readers() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in source::sources_under(&source::rust_root().join("web/ui/src")) {
        let text = source::read(&path);
        let lines: Vec<&str> = text.lines().collect();
        for index in 0..lines.len() {
            let Some(deciding) = decision_line(&lines, index) else {
                continue;
            };
            out.insert(format!(
                "{} {}",
                source::relative(&path),
                enclosing_fn(&text, deciding + 1)
            ));
        }
    }
    out
}

/// Every file under `rust/`, except the harness itself, whose **code** mentions `literal`.
///
/// The harness is exempt because a fixture legitimately names a code it must prove is refused
/// (`rust/test-harness/src/actors.rs` does exactly that); production code may not.
fn files_mentioning(root: &Path, literal: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in source::sources_under(root) {
        let relative = source::relative(&path);
        if relative.starts_with("rust/test-harness/") {
            continue;
        }
        if source::read(&path)
            .lines()
            .any(|line| source::code_of(line).contains(literal))
        {
            out.insert(relative);
        }
    }
    out
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-010); the file and the assay use it.
fn arch_boundary_010__entitlement_owns_action_screen_authorization() {
    // 1. THE DETECTORS, against planted samples. A scanner that cannot tell a decision from a declaration would report
    //    a clean tree and pass, which is the failure this harness exists to prevent.
    let planted: [(Vec<&str>, bool); 7] = [
        (
            vec!["        grants.entitlement_codes.iter().any(|code| code == action)"],
            true,
        ),
        (vec!["    pub entitlement_codes: Vec<String>,"], false),
        (
            vec!["        entitlement_codes: strings(\"entitlementCodes\"),"],
            false,
        ),
        (
            vec![
                "            entitlement_codes: model::security::ROOT_ONLY_ACTIONS",
                "                .iter()",
                "                .map(|code| code.to_string())",
                "                .collect(),",
            ],
            false,
        ),
        (
            vec![
                "        actor",
                "            .entitlement_codes",
                "            .iter()",
                "            .any(|held| held == entry.entitlement)",
            ],
            true,
        ),
        (
            vec!["        actor.holds_entitlement(entry.entitlement)"],
            true,
        ),
        (
            // The definition of `holds_entitlement`, three lines under the definition of `holds_authority`: the
            // authority check's window reaches the next function's *name*, and a name is not a decision.
            vec![
                "    pub(crate) fn holds_authority(&self, code: &str) -> bool {",
                "        code.is_empty() || self.authority_codes.iter().any(|held| held == code)",
                "    }",
                "",
                "    pub(crate) fn holds_entitlement(&self, code: &str) -> bool {",
            ],
            false,
        ),
    ];
    for (sample, expected) in &planted {
        assert_eq!(
            decision_line(sample, 0).is_some(),
            *expected,
            "the reader detector must {} `{}`",
            if *expected { "count" } else { "ignore" },
            sample[0].trim()
        );
    }

    // 2. THE CATALOG IS THE ONE NAMER. An action the authorize endpoint cannot name cannot be decided, so the catalog
    //    is the vocabulary every gate has to come from — and the anonymous door's actions have to be in it too.
    let declared = entitlement_catalog();
    let catalog: BTreeMap<String, String> = declared.iter().cloned().collect();
    assert!(
        catalog.len() >= CATALOG_FLOOR,
        "the catalog holds {} decidable actions, below the {CATALOG_FLOOR} floor: a scanner that read almost nothing \
         must fail instead of flattering the tree",
        catalog.len()
    );
    assert_eq!(
        declared.len(),
        catalog.len(),
        "an action is named twice in web/src/security/entitlement_catalog.rs: one fact, one row"
    );
    let kinds: BTreeSet<&str> = catalog.values().map(String::as_str).collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["command", "query"]),
        "every catalogued action is a query or a command; a third kind would be a new authorization model"
    );
    for code in [ENTITLEMENT_MANAGE, ROLE_MANAGE] {
        assert!(
            catalog.contains_key(code),
            "`{code}` is a root-only action: it must stay decidable"
        );
    }
    let public = public_read_actions();
    assert_eq!(
        public,
        PUBLIC_READ_ACTIONS
            .iter()
            .map(|code| code.to_string())
            .collect::<BTreeSet<String>>(),
        "the server's anonymous-door actions changed; a public read is an action like any other and the pin moves with \
         it deliberately"
    );
    for code in &public {
        assert!(
            catalog.contains_key(code),
            "the anonymous door authorizes `{code}`, which the catalog cannot name"
        );
    }

    // 3. EVERY SCREEN'S GATE IS DECIDABLE. The registry is the whole screen surface, and a gate on a code the catalog
    //    does not contain is a screen only ROOT can ever reach — an access decision nobody can diagnose.
    let gates = registry_gates();
    assert_eq!(
        gates.len(),
        ENTRY_COUNT,
        "the registry's screen count changed: a screen was added to or removed from web/ui/src/app/registry.rs, and \
         this contract's pins move with it"
    );
    let ungated = gates
        .iter()
        .filter(|gate| gate.entitlement.is_empty())
        .count();
    assert_eq!(
        ungated, UNGATED_ENTRIES,
        "the number of entries that require no entitlement changed: a screen that silently dropped its gate is exactly \
         what an unpinned gate vocabulary cannot see"
    );
    let entitlements: BTreeSet<&str> = gates
        .iter()
        .map(|gate| gate.entitlement.as_str())
        .filter(|code| !code.is_empty())
        .collect();
    assert_eq!(
        entitlements,
        BTreeSet::from(SCREEN_ENTITLEMENTS),
        "the entitlement vocabulary the registry gates screens on changed; adding or removing a gated screen is a \
         deliberate edit to this pin"
    );
    for code in &entitlements {
        assert!(
            catalog.contains_key(*code),
            "web/ui/src/app/registry.rs gates a screen on `{code}`, which \
             web/src/security/entitlement_catalog.rs cannot name: the authorize endpoint refuses what it cannot \
             name, so no role could ever be granted it and only ROOT would reach the screen"
        );
    }

    // 4. THE AUTHORITY HALF. Each entry is also gated on a legacy authority code. That half is not a catalog: it is
    //    pinned here so a new gate code cannot appear unnoticed, and the one code no catalog action covers is recorded
    //    as debt rather than hidden.
    let authorities: BTreeSet<&str> = gates
        .iter()
        .map(|gate| gate.authority.as_str())
        .filter(|code| !code.is_empty())
        .collect();
    assert_eq!(
        authorities,
        BTreeSet::from(SCREEN_AUTHORITIES),
        "the authority vocabulary the registry gates screens on changed; it is a second, legacy gate beside the \
         entitlement and it moves only deliberately"
    );
    let undecidable: BTreeSet<&str> = authorities
        .iter()
        .copied()
        .filter(|code| !catalog.contains_key(*code))
        .collect();
    assert_eq!(
        undecidable,
        BTreeSet::from(AUTHORITIES_WITHOUT_AN_ACTION),
        "the authority codes that name no catalogued action changed. `settings.read` (the Security screen) is the known \
         debt: half of that screen's gate is a code the authorize endpoint cannot decide. This pin may only shrink — a \
         new entry here means a new gate built on a vocabulary the server cannot answer"
    );

    // 5. THE READERS. Four places in `web/ui/src` decide from an entitlement list. A fifth is a second adjudicator: it
    //    is how a screen ends up offering what the API refuses.
    let readers = entitlement_readers();
    assert_eq!(
        readers,
        ENTITLEMENT_READERS
            .iter()
            .map(|site| site.to_string())
            .collect::<BTreeSet<String>>(),
        "the places that decide access from an entitlement list changed. `registry.rs surface_visible` reads the list \
         inline instead of calling `Actor::holds_entitlement` (a second copy of one rule, equivalent today) — if that \
         refactor lands, this pin is the record of it"
    );
    let ui_sources = source::sources_under(&source::rust_root().join("web/ui/src")).len();
    assert!(
        ui_sources >= UI_SOURCE_FLOOR,
        "the reader scan walked {ui_sources} files under web/ui/src, below the {UI_SOURCE_FLOOR} floor: a scan that \
         found almost nothing would pass this test while checking nothing"
    );
    assert!(
        source::read(&in_repo("web/ui/src/app/screen.rs")).contains("model::security::is_root_only("),
        "the portal offers a root-only action by asking the domain (`model::security::is_root_only`), never by \
         re-typing the two codes"
    );

    // 6. ONE WRITER FOR THE ROOT-ONLY CODES. A retyped literal is how the server and the portal drift apart: renaming
    //    the constant in the domain once must move every site, and a copy would not move.
    for code in [ENTITLEMENT_MANAGE, ROLE_MANAGE] {
        assert_eq!(
            files_mentioning(&source::rust_root(), code),
            BTreeSet::from(["middle/model/src/security.rs".to_string()]),
            "the root-only action `{code}` is written as a string literal outside \
             middle/model/src/security.rs; name it through `model::security` instead, so the two readers cannot \
             disagree about what it says"
        );
    }
    let catalog_text = source::read(&in_repo("web/src/security/entitlement_catalog.rs"));
    for constant in ["ENTITLEMENT_MANAGE", "ROLE_MANAGE"] {
        assert_eq!(
            catalog_text
                .matches(&format!("model::security::{constant}"))
                .count(),
            1,
            "the catalog names `{constant}` once, through the domain constant"
        );
    }
}
