//! UI.ENTITLEMENT — Power User no TECH (TST-UI-ENTITLEMENT-004).
//!
//! Contract: **`tech.access` does not open TECH to anybody but ROOT.** `web/ui/src/app/registry.rs::surface_visible` is
//! the rule, and it is the only place in the application where a **level** rather than a granted code decides whether a
//! whole operating world exists:
//!
//! ```text
//! Surface::Tech => (Some(Level::Root), "tech.access"),
//! …
//! if !actor.holds_authority(access_authority) { return false; }
//! if min_level.is_some_and(|required| actor.level() < required) { return false; }
//! ```
//!
//! TECH is the storyboard, the flight recorder and the design lab — the machinery this system runs on. The gate is
//! deliberately two-part: the authority `tech.access` **and** the ROOT level. The second half is what this contract pins,
//! because it is the half that a grant cannot buy.
//!
//! Four claims, each falsifiable on its own:
//!
//!   1. **A BUSINESS_POWER_USER HOLDING BOTH HALVES STILL GETS NO TECH.** The fixture is built to be maximally
//!      privileged below ROOT: internal, at the highest non-ROOT level, holding `tech.access` as BOTH an authority and
//!      an entitlement. Everything the grant vocabulary can express is present, and TECH is still absent. A test that
//!      only withheld the code would pass against a surface rule that checked grants alone.
//!   2. **THE ORDERING IS THE GATE, NOT A SPECIAL CASE.** `Level` is `Guest < User < BusinessPowerUser < Root`, and
//!      TECH's requirement is `Some(Level::Root)`. Asserting the ordering pins that the rule is a comparison: a future
//!      level inserted below Root must also be refused, and one inserted above must not exist.
//!   3. **THE GATE IS THE SURFACE'S, NOT THE ITEM'S.** The TECH rail entries ask only for `tech.access`, so an actor
//!      holding it passes the ITEM gate and is still kept out by the SURFACE gate. This is the distinction the whole
//!      contract turns on, and it is invisible unless asserted: both rules read the same codes and only the level
//!      separates them.
//!   4. **REFUSING TECH DOES NOT COST THE REST OF THE PORTAL.** A rule that hid everything to an actor below ROOT would
//!      pass 1-3 and lock the operator out of their own screens. The Power User keeps every other world.
//!
//! Level: L1 Component — the registry's surface-visibility rule, driven with real `Actor` projections. No database, no
//! network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_entitlement__004__power_user_no_tech

use ui::app::registry;
use ui::model::Surface;
use ui::navigation::{Actor, Level};

/// A BUSINESS_POWER_USER as privileged as the vocabulary allows: internal, the highest level below ROOT, holding
/// `tech.access` as an authority AND as an entitlement, plus every other entitlement the registry asks for anywhere.
///
/// If TECH is reachable from this actor, the gate is grants alone and this contract has failed to catch it.
fn maximally_privileged_power_user() -> Actor {
    let mut entitlement_codes: Vec<String> = registry::ENTRIES
        .iter()
        .filter(|entry| !entry.entitlement.is_empty())
        .map(|entry| entry.entitlement.to_string())
        .collect();
    // `tech.access` is both an entitlement and, for TECH itself, an authority — hold both so nothing is missing.
    if !entitlement_codes.iter().any(|code| code == "tech.access") {
        entitlement_codes.push("tech.access".into());
    }
    let mut authority_codes: Vec<String> = registry::ENTRIES
        .iter()
        .filter(|entry| !entry.authority.is_empty())
        .map(|entry| entry.authority.to_string())
        .collect();
    if !authority_codes.iter().any(|code| code == "tech.access") {
        authority_codes.push("tech.access".into());
    }
    Actor {
        level: Some(Level::BusinessPowerUser),
        account_type: "internal".into(),
        authority_codes,
        entitlement_codes,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ENTITLEMENT-004); the file and the assay use it.
fn ui_entitlement__004__power_user_no_tech() {
    let power_user = maximally_privileged_power_user();

    // 0. THE FIXTURE IS AS PRIVILEGED AS THE VOCABULARY ALLOWS. Everything below is worthless unless this holds, so it
    //    is asserted first and in full: if TECH is reachable from an actor holding every code the registry mentions, the
    //    level gate is gone.
    assert_eq!(
        power_user.level,
        Some(Level::BusinessPowerUser),
        "the fixture must be the highest level below ROOT"
    );
    assert_ne!(
        power_user.level,
        Some(Level::Root),
        "the fixture is not a Power User if it is ROOT — that actor is TST-UI-ENTITLEMENT-005's subject"
    );
    assert_eq!(
        power_user.account_type, "internal",
        "the fixture must be internal; an external account is refused everything and would prove nothing about levels"
    );
    assert!(
        power_user.authority_codes.iter().any(|code| code == "tech.access"),
        "the fixture must hold `tech.access` as an AUTHORITY — TECH's surface rule asks for it first"
    );
    assert!(
        power_user.entitlement_codes
            .iter()
            .any(|code| code == "tech.access"),
        "the fixture must hold `tech.access` as an ENTITLEMENT — otherwise this test would only be testing a missing grant"
    );

    // 1. NO TECH. The claim itself, at the level the fixture claims to be.
    assert!(
        !registry::visible_surfaces(&power_user).contains(&Surface::Tech),
        "a BUSINESS_POWER_USER holding `tech.access` as both an authority and an entitlement is offered TECH — the \
         ROOT-only surface rule is gone, and no grant may stand in for the level again"
    );
    // At every level below ROOT, with the same codes. A rule that special-cased BusinessPowerUser would pass the check
    // above and fail this one.
    for level in [Level::Guest, Level::User, Level::BusinessPowerUser] {
        let actor = Actor {
            level: Some(level),
            ..power_user.clone()
        };
        assert!(
            !registry::visible_surfaces(&actor).contains(&Surface::Tech),
            "an internal {level:?} holding every code the registry names is offered TECH"
        );
    }

    // 2. THE ORDERING IS THE GATE. `Level` derives `PartialOrd`, and `surface_visible` writes the rule as
    //    `actor.level() < required` with `required = Level::Root` for TECH. Asserting the ordering pins that the rule is
    //    a comparison rather than a named special case: a level added below Root is refused by the same comparison, and
    //    this test says so rather than leaving it to be discovered.
    assert!(
        Level::Guest < Level::User
            && Level::User < Level::BusinessPowerUser
            && Level::BusinessPowerUser < Level::Root,
        "the level ordering moved: TECH's gate is `actor.level() < Level::Root`, so the ranking is the rule and every \
         comparison in this test depends on it"
    );
    // An actor with no level at all resolves to GUEST and is refused — the fail-closed direction (`Actor::level`
    // unwraps to `Guest`, never to something higher).
    let level_less = Actor {
        level: None,
        ..power_user.clone()
    };
    assert!(
        !registry::visible_surfaces(&level_less).contains(&Surface::Tech),
        "an actor with no level is offered TECH — a missing level must fail closed to GUEST, never read as access"
    );
    // An unrecognised level string fails closed too. This is the boundary the portal receives it at:
    // `navigation::actor_from_json` returns an empty actor for an unparsable payload, and `Level::parse` maps anything
    // it does not recognise to GUEST.
    assert_eq!(
        Level::parse("TECHNICAL_SUPERUSER"),
        Level::Guest,
        "an unrecognised level string must fail closed to GUEST — a string nobody recognises must not read as access"
    );
    assert_eq!(
        Level::parse("ROOT"),
        Level::Root,
        "the level string the server sends must still parse to the level it names"
    );

    // 3. THE GATE IS THE SURFACE'S, NOT THE ITEM'S. The TECH rail entries ask only for `tech.access`, so this actor
    //    PASSES the item gate — and is still kept out by the surface gate. Asserting both halves is the only way this
    //    contract distinguishes "TECH is ROOT-only" from "TECH's items happen to be gated".
    let tech_rail: std::collections::BTreeSet<&'static str> =
        registry::rail_items(Surface::Tech, &power_user)
            .into_iter()
            .map(|(_, entry)| entry.key)
            .collect();
    assert!(
        tech_rail.contains("tech"),
        "the TECH Cockpit is no longer offered to an actor holding `tech.access` as an authority and an entitlement — \
         the item gate and the surface gate have converged, and the ROOT-only rule must be re-pinned against whatever \
         changed"
    );
    assert!(
        tech_rail.contains("storyboard") && tech_rail.contains("design-lab"),
        "the TECH rail no longer offers Story Board and UI Lab to this actor — the item gate moved"
    );
    // And the capsule that would lead there is absent, so the operator is never offered the world whose contents they
    // would be kept out of.
    let worlds = registry::visible_surfaces(&power_user);
    assert!(
        !worlds.contains(&Surface::Tech) && !worlds.contains(&Surface::Site),
        "the top nav must offer neither TECH nor the public SITE as an operating world — Site returns false from \
         `surface_visible` by design"
    );

    // 4. REFUSING TECH COSTS NOTHING ELSE. A rule that hid every world from a non-ROOT actor would satisfy 1-3 while
    //    locking the operator out of the portal entirely. This is the availability half of the same contract.
    assert!(
        worlds.contains(&Surface::Core),
        "a maximally privileged Power User is not offered CORE — the surface rule refused everything rather than TECH"
    );
    for surface in [
        Surface::Core,
        Surface::Accounting,
        Surface::Marketing,
        Surface::Ops,
        Surface::Support,
    ] {
        assert!(
            worlds.contains(&surface),
            "a BUSINESS_POWER_USER holding every code the registry names is not offered {} — only TECH is level-gated, \
             so refusing this world means the rule has grown",
            surface.key()
        );
        assert!(
            !registry::rail_items(surface, &power_user).is_empty(),
            "the {} world is offered but its rail is empty — the operator is sent to a world with no destinations",
            surface.key()
        );
    }
    assert!(
        !worlds.contains(&Surface::Tech) && worlds.len() == 5,
        "the Power User is offered {} worlds; the registry declares six operating surfaces plus Site, so the count \
         moves only if a world is added or removed",
        worlds.len()
    );

    // 5. TECH'S OWN DECLARATION IS WHAT SAYS SO. `surface_visible` special-cases exactly one surface with a level, and
    //    this test's fixture is written against TECH. Pinned so a second level-gated world, or TECH losing its
    //    requirement, is a deliberate change to this contract rather than a silent drift.
    let tech_entries: Vec<&str> = registry::ENTRIES
        .iter()
        .filter(|entry| entry.surface == Surface::Tech)
        .map(|entry| entry.key)
        .collect();
    assert_eq!(
        tech_entries,
        vec!["tech", "storyboard", "design-lab", "story-record", "trace-record"],
        "the set of TECH screens changed, in registry table order: the fixture above is written against these, and a new \
         one must be added to it deliberately"
    );
    for key in ["tech", "storyboard", "design-lab"] {
        let entry = registry::by_key(key).unwrap_or_else(|| panic!("{key} is a registered screen"));
        assert_eq!(
            entry.authority, "tech.access",
            "{key} asks for a different authority than `tech.access`; the item gate this test separates from the surface \
             gate has moved"
        );
    }
}
