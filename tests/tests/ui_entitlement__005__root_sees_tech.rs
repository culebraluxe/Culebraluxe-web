//! UI.ENTITLEMENT — ROOT sees TECH (TST-UI-ENTITLEMENT-005).
//!
//! Contract: **ROOT reaches every operating world, and TECH with the rest.** `registry::visible_surfaces` filters
//! `SURFACE_ORDER` through `surface_visible`, and `surface_visible` opens TECH for exactly one actor: one that is
//! internal, holds the `tech.access` authority, is at `Level::Root`, and can reach something in it. This contract is
//! the other half of TST-UI-ENTITLEMENT-004 — that story pins the refusal, this one pins that the refusal is not a
//! blanket denial. Together they are the whole rule: a level gate that refused ROOT too would satisfy 004 forever while
//! making the storyboard, the flight recorder and the design lab unreachable by anybody.
//!
//! Four claims:
//!
//!   1. **ROOT SEES EVERY WORLD.** All six operating surfaces, in the registry's declared order, and not `Site` — which
//!      returns false from `surface_visible` by design, because the public site is not an operating world.
//!   2. **ROOT SEES EVERY RAIL.** Every surface's rail is non-empty for ROOT, so no capsule leads nowhere.
//!   3. **IT IS THE LEVEL, NOT A GRANT.** ROOT is shown holding **no** entitlements at all, so every item it can see, it
//!      sees because `Actor::holds_entitlement` satisfies ROOT (`code.is_empty() || self.is_root() || held`). This is
//!      what makes the exemption a real one rather than a fixture that quietly granted everything.
//!   4. **THE AUTHORITY IS STILL ASKED.** The rules check the authority first and do not exempt ROOT from it — the same
//!      arrangement the registry's own test comment records. An actor at ROOT *without* `tech.access` is refused TECH,
//!      so ROOT's reach is not unbounded: this is a UI-visibility table, and the server authorizes every request.
//!
//! The limit, stated so a green run is not over-read: this proves what the chrome will draw. It does not prove what
//! ROOT may do — `registry.rs` says so at the head of `navigation.rs` ("THIS IS UI VISIBILITY, NOT AUTHORIZATION"), and
//! the server's own policy tests own that. An empty rail is not a closed door.
//!
//! Level: L1 Component — the registry's surface-visibility rule, driven with real `Actor` projections. No database, no
//! network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_entitlement__005__root_sees_tech

use ui::app::registry;
use ui::model::Surface;
use ui::navigation::{Actor, Level};

/// ROOT as the portal layout hands it over: the level AND the authorities its roles carry. No entitlements — ROOT does
/// not need them, and holding none is what makes the exemption observable rather than assumed.
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

/// The operating surfaces, in the order the registry declares them for the top nav.
const OPERATING_WORLDS: [Surface; 6] = [
    Surface::Core,
    Surface::Accounting,
    Surface::Marketing,
    Surface::Ops,
    Surface::Support,
    Surface::Tech,
];

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ENTITLEMENT-005); the file and the assay use it.
fn ui_entitlement__005__root_sees_tech() {
    let root_actor = root();

    // 0. THE FIXTURE IS ROOT, AND IT IS ROOT BY THE LEVEL ALONE.
    assert_eq!(
        root_actor.level,
        Some(Level::Root),
        "this fixture's subject is ROOT; anything else makes every assertion below meaningless"
    );
    assert_eq!(
        root_actor.account_type, "internal",
        "ROOT must be internal — `surface_visible` requires `actor.internal()`, so an external ROOT would be refused \
         everything and would prove nothing about ROOT"
    );
    assert!(
        root_actor.entitlement_codes.is_empty(),
        "ROOT must hold no entitlements: it sees everything because `holds_entitlement` satisfies ROOT, and a fixture \
         that granted everything would hide the exemption this contract exists to pin"
    );
    assert!(
        root_actor.authority_codes.iter().any(|code| code == "tech.access"),
        "ROOT's roles must carry `tech.access` — TECH's surface rule asks for it before it asks anything else"
    );

    // 1. ROOT SEES EVERY WORLD, IN THE DECLARED ORDER. Not merely "most of them": the full list, in sequence, because
    //    the order is what the top nav is drawn in and an operator reads it as a sequence.
    let worlds = registry::visible_surfaces(&root_actor);
    assert_eq!(
        worlds,
        OPERATING_WORLDS.to_vec(),
        "ROOT's operating worlds changed: the top nav is drawn in this order, so a world added, removed or reordered \
         must move this list deliberately"
    );
    assert!(
        worlds.contains(&Surface::Tech),
        "ROOT is not offered TECH — the one surface whose gate is a level must be open to the level that satisfies it"
    );

    // 2. EVERY WORLD LEADS SOMEWHERE. A capsule with an empty rail is a destination that opens onto nothing, and it is
    //    the failure a "visible" world can have without being visibly broken.
    for surface in OPERATING_WORLDS {
        let rail = registry::rail_items(surface, &root_actor);
        assert!(
            !rail.is_empty(),
            "ROOT is offered the {} world with an empty rail — the capsule leads to a world with no destinations",
            surface.key()
        );
        for (_, entry) in &rail {
            assert_eq!(
                entry.surface,
                surface,
                "{} is on the {} rail but declares another surface",
                entry.key,
                surface.key()
            );
        }
    }
    // TECH specifically, with its destinations named — this is the screen the story is about, so it is checked by name
    // rather than by count.
    let tech: Vec<&str> = registry::rail_items(Surface::Tech, &root_actor)
        .into_iter()
        .map(|(_, entry)| entry.key)
        .collect();
    assert_eq!(
        tech,
        vec!["tech", "storyboard", "design-lab"],
        "ROOT's TECH rail changed: the Story Board, the flight recorder's parent and the design lab are what this \
         surface exists to reach"
    );
    // The full CORE rail, so a regression that emptied one world is visible as a list rather than as a count.
    let core: Vec<&str> = registry::rail_items(Surface::Core, &root_actor)
        .into_iter()
        .map(|(_, entry)| entry.key)
        .collect();
    assert_eq!(
        core,
        vec![
            "dashboard",
            "clients",
            "projects",
            "deals",
            "cabinet",
            "workflows",
            "forms",
            "seller-strategy",
        ],
        "ROOT's CORE rail changed"
    );

    // 3. IT IS THE LEVEL, NOT A GRANT. Dropping ROOT one level removes TECH and only TECH — which is the whole content
    //    of TST-UI-ENTITLEMENT-004, asserted here from the other side so the two stories cannot drift apart.
    //
    //    The actor is given **every entitlement the registry asks for**, so the level is the only thing that differs.
    //    That matters: `surface_visible` requires an actor to reach at least one rail item, and it grants ROOT an
    //    exemption from that requirement (`actor.is_root() || …entitlement_codes…`). A ROOT-shaped fixture with no
    //    entitlements would therefore see NOTHING one level down — not "everything but TECH" — and would prove only that
    //    the ROOT exemption exists, never that the level gate is what closed TECH.
    let every_entitlement: Vec<String> = registry::ENTRIES
        .iter()
        .filter(|entry| !entry.entitlement.is_empty())
        .map(|entry| entry.entitlement.to_string())
        .collect();
    let one_down = Actor {
        level: Some(Level::BusinessPowerUser),
        entitlement_codes: every_entitlement.clone(),
        ..root_actor.clone()
    };
    assert!(
        one_down.entitlement_codes.iter().any(|code| code == "tech.access"),
        "this actor must hold `tech.access` as an entitlement — TECH must be kept out by the LEVEL, not by a missing \
         grant, or this test is not testing the level gate"
    );
    assert_eq!(
        registry::visible_surfaces(&one_down),
        OPERATING_WORLDS[..5].to_vec(),
        "one level below ROOT the only world that goes is TECH; anything else means the level gate has spread"
    );
    assert!(
        !registry::visible_surfaces(&one_down).contains(&Surface::Tech),
        "a BUSINESS_POWER_USER is offered TECH — the rule that story 004 pins is gone"
    );

    // 4. THE AUTHORITY IS STILL ASKED. The rules check the authority first and do not exempt ROOT from it. This is what
    //    keeps ROOT's reach from being unbounded, and it is the honest counterpart of "this is visibility, not
    //    authorization": even the most privileged actor is filtered by the same table, and the server authorizes every
    //    request regardless of what this one draws.
    let root_without_tech = Actor {
        authority_codes: ["portal.read", "deal.read", "settings.read"]
            .map(String::from)
            .to_vec(),
        ..root_actor.clone()
    };
    assert!(
        !registry::visible_surfaces(&root_without_tech).contains(&Surface::Tech),
        "ROOT without the `tech.access` authority is offered TECH — the rules check the authority before the level, and \
         ROOT is not exempt from it"
    );
    // And the rest of the portal is untouched: the missing authority costs TECH and nothing else.
    assert_eq!(
        registry::visible_surfaces(&root_without_tech),
        OPERATING_WORLDS[..5].to_vec(),
        "losing `tech.access` cost more than TECH"
    );
    // An external account is refused everything whatever its level, so ROOT's reach is bounded by the account type as
    // well as by the level — the negative case for the whole story.
    let root_but_external = Actor {
        account_type: "guest".into(),
        ..root_actor.clone()
    };
    assert!(
        registry::visible_surfaces(&root_but_external).is_empty(),
        "an external account whose level says ROOT is offered the operator navigation — `internal()` is the gate, and a \
         level string is not an account type"
    );

    // 5. THE DECLARATION THE FIXTURE IS WRITTEN AGAINST. `surface_visible` names exactly one surface with a level
    //    requirement, and TECH's own entries name `tech.access` — both are what sections 1 and 4 turn on, so both are
    //    pinned rather than left implicit.
    let tech_entry = registry::by_key("tech").expect("the TECH Cockpit is registered");
    assert_eq!(
        tech_entry.authority, "tech.access",
        "the TECH Cockpit's authority changed; the fixture above is written against it"
    );
    assert_eq!(
        tech_entry.entitlement, "tech.access",
        "the TECH Cockpit's entitlement changed; ROOT sees it through `is_root`, so this pin is what shows the \
         exemption is what is doing the work"
    );
    // Every TECH **rail** entry asks for the same authority, so none of them can be reached by an actor the world refused.
    // The drill-ins (`story-record`, `trace-record`) are deliberately excluded: they are `Menu::None`, reached from a
    // parent inside TECH, and they carry `portal.read` — the same arrangement every drill-in in the registry uses.
    // What must hold is that the three destinations the rail itself offers are gated as one world.
    for key in ["tech", "storyboard", "design-lab"] {
        let entry = registry::by_key(key).unwrap_or_else(|| panic!("{key} is a registered screen"));
        assert_eq!(
            entry.authority, "tech.access",
            "{key} asks for an authority other than `tech.access`, so TECH is no longer gated as one world"
        );
    }
    for key in ["story-record", "trace-record"] {
        let entry = registry::by_key(key).unwrap_or_else(|| panic!("{key} is a registered screen"));
        assert_eq!(
            entry.surface, Surface::Tech,
            "{key} is not a TECH screen any more, so the TECH surface this contract pins has changed"
        );
        assert!(
            entry.parent.is_some(),
            "{key} is a TECH screen with no parent, so it would be reachable from no list — a destination an operator \
             cannot be sent to by name"
        );
    }
}
