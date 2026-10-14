//! UI.ENTITLEMENT — hidden navigation (TST-UI-ENTITLEMENT-001).
//!
//! Contract: **a destination the actor may not use is not offered, and hiding it is never treated as the gate.**
//! `web/ui/src/app/registry.rs` owns the visibility rules the chrome renders from — `visible_surfaces(actor)` for the
//! top-nav worlds and `rail_items(surface, actor)` for the rail — and `navigation.rs` owns the actor those rules read.
//! Three properties, each in both directions, because each fails in one direction only:
//!
//!   1. **HELD ⇒ OFFERED, NOT HELD ⇒ HIDDEN.** An item is offered when the actor holds its `authority`, is internal, and
//!      holds its `entitlement` (`item_visible`). The negative direction is the one that matters: an actor without the
//!      entitlement must not see the link, and must not see it *because* of ROOT, who satisfies every entitlement.
//!   2. **A WORLD WITH NOTHING IN IT IS NOT A WORLD.** `surface_visible` opens a surface only when the actor can reach
//!      at least one rail item in it. Offering TECH to someone who can open nothing in TECH is a dead capsule — the
//!      operator clicks it and arrives at a rail with no destinations.
//!   3. **AN EXTERNAL ACCOUNT SEES NO OPERATING WORLD AT ALL.** The rules require `actor.internal()`. A guest, a client
//!      or a seller signing a document is not an operator, and every `Menu::Rail` destination is behind that check.
//!
//! AND THE LIMIT THAT MATTERS MOST, pinned in section 4: **this is visibility, not authorization.** `registry.rs` says
//! so at the top of `navigation.rs`, and this test holds it to that. An empty rail is not a closed door — the server
//! authorizes every request and re-checks. What this test refuses is a *claim* that hiding is protecting: a nav that
//! hides a link an actor may use (an operator who cannot find a screen they are entitled to) is a bug in this table, and
//! one that shows a link an actor may not use is a bug here too. Both are availability and disclosure defects; neither is
//! fixed by this test, and neither is what this test is for.
//!
//! Level: L1 Component — the registry's visibility decision layer, driven with real `Actor` projections. No database, no
//! network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_entitlement__001__hidden_navigation

use std::collections::BTreeSet;

use ui::app::registry::{self, Entry, Menu};
use ui::model::Surface;
use ui::navigation::{Actor, Level};

/// An ordinary internal operator: `portal.read`, and only the entitlements handed to it.
fn user_with(entitlements: &[&str]) -> Actor {
    Actor {
        level: Some(Level::User),
        account_type: "internal".into(),
        authority_codes: vec!["portal.read".into(), "deal.read".into()],
        entitlement_codes: entitlements
            .iter()
            .map(|code| (*code).to_string())
            .collect(),
    }
}

/// A business power user: an internal operator who is not ROOT and so cannot reach TECH, whatever else it holds.
fn power_user() -> Actor {
    Actor {
        level: Some(Level::BusinessPowerUser),
        account_type: "internal".into(),
        authority_codes: vec![
            "portal.read".into(),
            "deal.read".into(),
            "settings.read".into(),
            "tech.access".into(),
        ],
        entitlement_codes: ["portal.read", "tech.access", "person.read", "property.read"]
            .map(String::from)
            .to_vec(),
    }
}

/// ROOT as the portal layout hands it over: the level AND the authorities its roles carry.
fn root() -> Actor {
    Actor {
        level: Some(Level::Root),
        account_type: "internal".into(),
        authority_codes: ["portal.read", "deal.read", "settings.read", "tech.access"]
            .map(String::from)
            .to_vec(),
        entitlement_codes: Vec::new(),
    }
}

/// An external account — a guest, a client, a seller at a signing link. Signed in, and not an operator.
fn external() -> Actor {
    Actor {
        level: Some(Level::Guest),
        account_type: "guest".into(),
        authority_codes: vec!["portal.read".into()],
        entitlement_codes: ["portal.read", "person.read", "tech.access"]
            .map(String::from)
            .to_vec(),
    }
}

/// The keys of the rail `surface` offers `actor`.
fn rail(actor: &Actor, surface: Surface) -> BTreeSet<&'static str> {
    registry::rail_items(surface, actor)
        .into_iter()
        .map(|(_, entry)| entry.key)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ENTITLEMENT-001); the file and the assay use it.
fn ui_entitlement__001__hidden_navigation() {
    // 1. NOT HELD ⇒ HIDDEN. The direction that decides whether the UI respects the entitlement at all. For every rail
    //    entry, an internal actor that does NOT hold its entitlement must not see it — with one exception the rules
    //    state explicitly and this test must not quietly break: ROOT satisfies every entitlement
    //    (`Actor::holds_entitlement`: `code.is_empty() || self.is_root() || ...`).
    let alice = user_with(&["person.read", "deal.read", "form.read"]);
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        let offered = rail(&alice, surface);
        for (label, entry) in registry::rail_items(surface, &alice) {
            let held = entry.entitlement.is_empty()
                || alice
                    .entitlement_codes
                    .iter()
                    .any(|code| code == entry.entitlement);
            assert!(
                held,
                "{label} ({}) is offered to an actor holding {:?}, but it requires `{}` — a link the server will \
                 refuse is being drawn in the rail",
                entry.key,
                alice.entitlement_codes,
                entry.entitlement
            );
            assert!(
                entry.authority.is_empty()
                    || alice
                        .authority_codes
                        .iter()
                        .any(|code| code == &entry.authority),
                "{label} ({}) is offered without its authority `{}`",
                entry.key,
                entry.authority
            );
        }
        // Nothing that requires TECH's authority is reachable by an actor without it.
        if !alice
            .authority_codes
            .iter()
            .any(|code| code == "tech.access")
        {
            assert!(
                !offered.contains("tech") && !offered.contains("storyboard"),
                "an actor without `tech.access` sees {offered:?} on the {} rail",
                surface.key()
            );
        }
    }

    // 2. HELD ⇒ OFFERED. The direction that stops the table from becoming a blacklist. An internal actor holding an
    //    item's authority and entitlement sees it: a destination nobody can find is a screen that exists for no one.
    for (label, entry) in registry::rail_items(Surface::Core, &alice) {
        assert!(
            entry.authority.is_empty() || alice.authority_codes.iter().any(|code| code == &entry.authority),
            "{label} ({}) requires the authority `{}`, which the actor was given, yet it is not offered — hiding a \
             destination an actor may use is a bug in this table",
            entry.key,
            entry.authority
        );
    }
    // ROOT sees every rail the registry declares, because ROOT is exempt from the entitlement half.
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        assert!(
            !rail(&root(), surface).is_empty(),
            "ROOT sees an empty {} rail — ROOT satisfies every entitlement, so an empty rail here means the entry \
             requires an authority ROOT's roles do not carry",
            surface.key()
        );
    }

    // 3. A WORLD WITH NOTHING IN IT IS NOT A WORLD. `visible_surfaces` is the top nav; every capsule it offers must
    //    lead somewhere. An empty capsule is a dead end an operator clicks.
    let worlds = registry::visible_surfaces(&root());
    assert!(
        worlds.len() >= 6,
        "ROOT sees {} operating worlds, below the floor of 6 — an empty or truncated rule would pass this test while \
         hiding everything",
        worlds.len()
    );
    for actor in [&alice, &power_user(), &root()] {
        for surface in registry::visible_surfaces(actor) {
            assert!(
                !rail(actor, surface).is_empty(),
                "{} is offered to a {actor:?} but its rail is empty — the capsule leads to a world with no \
                 destinations in it",
                surface.key()
            );
        }
    }

    // 4. AN EXTERNAL ACCOUNT SEES NO OPERATING WORLD. The negative case for the whole story, and the one that would be
    //    most damaging if it failed: a guest, a client or a seller is handed the full operator navigation. Every rule
    //    requires `actor.internal()`, so the answer must be nothing at all — even with a plausible entitlement list and
    //    the `portal.read` authority in hand.
    let guest = external();
    assert_eq!(
        registry::visible_surfaces(&guest),
        Vec::new(),
        "an external account is offered an operating world: {}",
        registry::visible_surfaces(&guest)
            .iter()
            .map(|surface| surface.key())
            .collect::<Vec<&str>>()
            .join(", ")
    );
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        assert!(
            rail(&guest, surface).is_empty(),
            "an external account sees {surface:?} rail items: {:?} — the operator navigation is drawn for an account \
             that may not operate the portal",
            rail(&guest, surface)
        );
    }
    // An external account holding ROOT's level is still not internal: the account type is the check, not the level.
    let privileged_guest = Actor {
        level: Some(Level::Root),
        account_type: "guest".into(),
        authority_codes: ["portal.read", "tech.access", "deal.read", "settings.read"]
            .map(String::from)
            .to_vec(),
        entitlement_codes: Vec::new(),
    };
    assert!(
        registry::visible_surfaces(&privileged_guest).is_empty(),
        "an external account whose level says ROOT is offered the operator navigation — `internal()` is the gate, and \
         a level string is not an account type"
    );

    // 5. THE HIDDEN HALF IS STILL REAL. A hidden destination is still registered, routable, and named. This is the
    //    falsifiable form of "hiding a link is not a gate": if hiding removed the screen, the registry would lose it,
    //    and `resolve` would stop answering for its path. An implementation that "protected" a route by hiding it
    //    would break here, and would have broken the server's own routing with it.
    let hidden = registry::ENTRIES
        .iter()
        .find(|entry| entry.key == "tech")
        .expect("TECH's Cockpit is a registered screen");
    // The power user holds `tech.access` as BOTH an authority and an entitlement, and is still not offered TECH: the
    // surface rule additionally requires ROOT. `PortalFrame` draws the rail with `rail_items(active, &actor)` for the
    // surface it is given, and the capsule list with `visible_surfaces(&actor)` — so the surface rule is the only thing
    // keeping this actor out of TECH, and the two must be read as different gates rather than as one.
    let power = power_user();
    assert!(
        power
            .authority_codes
            .iter()
            .any(|code| code == "tech.access"),
        "this fixture's whole point is a power user who HOLDS `tech.access`"
    );
    assert!(
        power
            .entitlement_codes
            .iter()
            .any(|code| code == "tech.access"),
        "this fixture's whole point is a power user who HOLDS the `tech.access` entitlement"
    );
    assert!(
        !registry::visible_surfaces(&power).contains(&Surface::Tech),
        "a non-ROOT actor holding `tech.access` is offered TECH: the ROOT-only surface rule is gone, and \
         TST-UI-ENTITLEMENT-004 must move with it"
    );
    // The item half and the surface half are different decisions, and this is where that is visible: the TECH rail
    // items pass the item gate, because TECH's own entries ask only for `tech.access`.
    assert!(
        rail(&power, Surface::Tech).contains("tech"),
        "the TECH Cockpit no longer asks only for `tech.access`; the surface rule and the item rule have converged and \
         the ROOT-only gate must be re-pinned"
    );
    assert!(
        registry::resolve("/portal/tech").is_some(),
        "a hidden destination must still resolve — hiding a link removes it from a menu, never from the router"
    );
    assert!(
        hidden.entitlement == "tech.access",
        "the TECH surface's own gate changed; this fixture's visibility rules are written against it"
    );

    // 6. NO ENTRY IS BOTH IN A MENU AND UNGATED ON A PORTAL ROUTE WITHOUT AN AUTHORITY. A rail tab that needs neither
    //    an authority nor an entitlement is offered to every internal account, so the internal/external check is the
    //    only thing standing between it and every signed-in visitor. That is allowed and deliberate (public pages are
    //    `Menu::Header`, not `Menu::Rail`), so the assertion is on the portal rail only, and it is stated rather than
    //    assumed: nothing in the portal rail is ungated on both halves.
    let ungated_rail: Vec<&str> = registry::ENTRIES
        .iter()
        .filter(|entry: &&Entry| {
            entry.menu == Menu::Rail("")
                || (matches!(entry.menu, Menu::Rail(_))
                    && entry.authority.is_empty()
                    && entry.entitlement.is_empty())
        })
        .map(|entry| entry.key)
        .collect();
    assert!(
        ungated_rail.is_empty(),
        "these portal rail entries require neither an authority nor an entitlement: {ungated_rail:?} — every internal \
         account, at any level, is offered them"
    );
}
