//! SEC.ENTITLEMENT — TECH (TST-SEC-ENTITLEMENT-008).
//!
//! Contract: **the TECH world is ROOT-only, and ROOT alone is not enough.** `visible_surfaces`
//! (`web/ui/src/app/registry.rs:302-308`) filters the registry's surfaces through `surface_visible`
//! (`:320-343`), which for TECH requires *both* halves of the gate — the authority code `tech.access`
//! (`holds_authority`, `web/ui/src/navigation.rs:74-76`: a signed-in actor must literally hold the code; ROOT
//! satisfies entitlements, not authority codes) **and** the `ROOT` level (the 3-field level ladder
//! `Guest < User < BusinessPowerUser < Root`, `:29-36`) — and then, like every other world, an internal account
//! (`:66-68`) with at least one reachable rail capsule (ROOT reaches all, `:78-80`).
//!
//! It is asserted through the production boundary — the registry's own `visible_surfaces` on real `Actor` values —
//! not through a copy of the rule. Each refusal is paired with its control: the same actor with the one field changed
//! *does* see TECH, so a green run cannot come from a filter that simply returns nothing.
//!
//! The negative cases are the subject: a `BusinessPowerUser` who holds `tech.access`, and a ROOT who holds the
//! entitlement but not the authority code, must both be refused TECH; and an external account given the ROOT level
//! and the authority code must reach no world at all. Each of those is the defect this contract exists to prevent —
//! the ops console (database diagnostics, app errors, engine state) shown to somebody who was not meant to have it.
//!
//! What this does not cover, stated so nobody reads more into a green run: this is UI *visibility*, and the server
//! authorizes every request independently (the shell's own doc says so, `web/ui/src/app/screen.rs:52-54`); this test
//! proves what the portal offers, never what the API would allow.
//!
//! Level: L0 Pure — the registry's pure visibility rule on constructed actors. No database, no network, no wasm.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__008__tech

use ui::app::registry::visible_surfaces;
use ui::model::Surface;
use ui::navigation::{Actor, Level};

const HARNESS: &str = "RegistryHarness/L0 Pure";

/// The six portal worlds in display order — the registry's own order, minus `Site`, which is not a portal surface.
fn all_worlds() -> Vec<Surface> {
    vec![
        Surface::Core,
        Surface::Accounting,
        Surface::Marketing,
        Surface::Ops,
        Surface::Support,
        Surface::Tech,
    ]
}

/// An actor as the page hands it over: a level, the two code lists, and the account type.
fn actor(
    level: Level,
    account_type: &str,
    authority_codes: &[&str],
    entitlement_codes: &[&str],
) -> Actor {
    Actor {
        level: Some(level),
        account_type: account_type.to_owned(),
        authority_codes: authority_codes.iter().map(|code| (*code).to_owned()).collect(),
        entitlement_codes: entitlement_codes
            .iter()
            .map(|code| (*code).to_owned())
            .collect(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-008); the file and the assay use it.
fn sec_entitlement_008__tech() {
    // 1. The only actor who reaches TECH: ROOT, internal, holding the authority code. ROOT reaches every capsule,
    //    so the list is the whole portal and the order is the registry's — SUPPORT before TECH.
    let root = actor(Level::Root, "internal", &["tech.access"], &[]);
    let worlds = visible_surfaces(&root);
    assert_eq!(
        worlds,
        all_worlds(),
        "{HARNESS}: ROOT holding tech.access sees every portal world, in display order"
    );
    assert!(
        worlds.contains(&Surface::Tech),
        "{HARNESS}: TECH is the console this contract is about"
    );
    assert!(
        !worlds.contains(&Surface::Site),
        "{HARNESS}: the public site is not a portal world and never appears in the portal's capsules"
    );

    // 2. ROOT alone is not enough: the authority code is the first half of the gate. The control is actor 1 — the
    //    same actor one code richer.
    let root_without_the_code = actor(Level::Root, "internal", &["settings.read"], &["tech.access"]);
    assert!(
        !visible_surfaces(&root_without_the_code).contains(&Surface::Tech),
        "{HARNESS}: a ROOT who holds the tech.access *entitlement* but not the authority code must not reach TECH \
         — the code, not the level, is what opens the console"
    );
    assert!(
        visible_surfaces(&root_without_the_code).contains(&Surface::Core),
        "{HARNESS}: the refusal must be TECH's, not a filter that returns nothing — this actor still sees Core"
    );

    // 3. The level is the second half: an internal power user who holds BOTH the code and the entitlement is
    //    refused, and the same actor lifted to ROOT is not. Nothing here is a missing entitlement.
    let power_user = actor(
        Level::BusinessPowerUser,
        "internal",
        &["tech.access"],
        &["tech.access"],
    );
    assert!(
        !visible_surfaces(&power_user).contains(&Surface::Tech),
        "{HARNESS}: a BusinessPowerUser holding tech.access (authority and entitlement) must not reach TECH"
    );
    let power_user_lifted = actor(
        Level::Root,
        "internal",
        &["tech.access"],
        &["tech.access"],
    );
    assert!(
        visible_surfaces(&power_user_lifted).contains(&Surface::Tech),
        "{HARNESS}: the same actor at ROOT does reach TECH — the level is the only difference and it is decisive"
    );

    // 4. The account rule outranks both halves: an external account given the ROOT level and the code reaches
    //    nothing at all, and the same actor as an internal account reaches TECH.
    let external_root = actor(Level::Root, "external", &["tech.access"], &["tech.access"]);
    assert_eq!(
        visible_surfaces(&external_root),
        Vec::new(),
        "{HARNESS}: an external account reaches no portal world, whatever level and codes it carries"
    );
    let external_root_internal = actor(Level::Root, "internal", &["tech.access"], &["tech.access"]);
    assert_eq!(
        visible_surfaces(&external_root_internal),
        all_worlds(),
        "{HARNESS}: the only field that changed is the account type, and with it the whole portal opens"
    );

    // 5. An empty actor — what an unparsable snapshot leaves behind (`actor_from_json` fails closed) — reaches no
    //    world, so a failure to read the page's projection can never grant the console.
    assert_eq!(
        visible_surfaces(&Actor::default()),
        Vec::new(),
        "{HARNESS}: no level, no codes, no account type reaches nothing"
    );
}
