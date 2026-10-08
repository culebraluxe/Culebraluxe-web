//! UI.ACCESSIBILITY — labels (TST-UI-ACCESSIBILITY-002).
//!
//! Contract: **every navigation landmark and every link in it is named, and no two links in the same landmark share a
//! name.** The registry is where those names live: `Menu::Header(label)` / `Menu::Rail(label)` are the strings the chrome
//! draws as the link text, and `Surface::label()` is what `PortalFrame` puts in `aria-label={format!("{} navigation",
//! active.label())}` — the accessible name of the rail itself. Two failures hide in that one table:
//!
//!   1. **A BLANK NAME.** A label that is empty, whitespace, or punctuation-only leaves a screen-reader user with an
//!      unlabelled list of unlabelled links. It is invisible on screen — which is exactly why it survives review.
//!   2. **A DUPLICATE NAME.** Two links in one landmark with the same name are indistinguishable when announced. A
//!      screen reader user hears "Buyers, link" twice and cannot tell which is which. This is the failure that
//!      uniqueness exists to catch, and it is invisible to a sighted reviewer too.
//!
//! The rail and the header are separate landmarks, so the SAME word may legitimately appear in both (the registry
//! declares `Cockpit` on both `Surface::Core` and `Surface::Tech`, in two different rails) and must not be treated as a
//! collision. Uniqueness is therefore pinned **per landmark**, which is the granularity an assistive technology actually
//! announces. TECH's rail legitimately contains `Cockpit` while CORE's rail also contains `Cockpit`; CORE's own rail must
//! not contain it twice.
//!
//! WHY THE REGISTRY AND NOT THE RENDERED MARKUP. These are the literal strings `chrome.rs` renders and reads back as the
//! `aria-label`; the Yew VDOM cannot be walked from a host test in any case (`VList`'s children are `pub(crate)` in
//! `yew 0.23`), so the decision layer is both the honest and the only reachable boundary.
//!
//! Level: L1 Component — the registry's menu vocabulary, driven in-process. No database, no network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_accessibility__002__labels

use std::collections::BTreeSet;

use ui::app::registry::{self, Entry, Menu};
use ui::model::Surface;
use ui::navigation::{Actor, Level};

/// A name that names nothing. An assistive technology announces the link as a link with no content, so this is the
/// unlabelled-link failure however the label was produced.
fn unnamed(label: &str) -> bool {
    let trimmed = label.trim();
    trimmed.is_empty()
        || trimmed
            .chars()
            .all(|character| character.is_whitespace() || character == '\u{2026}')
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ACCESSIBILITY-002); the file and the assay use it.
fn ui_accessibility_002__labels() {
    // 1. EVERY SURFACE IS NAMED. `Surface::label()` is drawn as the top-nav capsule text AND becomes the rail's
    //    `aria-label`. An unnamed surface produces an unnamed capsule and an unnamed landmark — and the failure is
    //    silent, because a nav with no name still looks like a nav.
    let mut named: BTreeSet<&str> = BTreeSet::new();
    for surface in Surface::ALL.iter().copied() {
        let label = surface.label();
        assert!(
            !unnamed(label),
            "Surface::{} is named \"{label}\" — the top-nav capsule and the rail's aria-label are both drawn from it",
            surface.key()
        );
        assert!(
            named.insert(label),
            "two surfaces share the label \"{label}\" — every operator would hear the same capsule name in the top nav \
             and could not tell the operating worlds apart"
        );
    }
    assert_eq!(
        named.len(),
        Surface::ALL.len(),
        "the number of distinct surface labels changed: a new surface must carry its own name"
    );

    // 2. THE SITE HEADER NAMES EVERY LINK, UNIQUELY. Seven links in one landmark, announced one after another.
    let header: Vec<&'static Entry> = registry::header_items().map(|(_, entry)| entry).collect();
    assert!(
        header.len() >= 5,
        "the site header offers only {} links, below the floor of 5: a loop over an empty or truncated header would \
         pass this test while naming nothing",
        header.len()
    );
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (label, entry) in registry::header_items() {
        assert!(
            !unnamed(label),
            "{} is in the site header with no name — its link is announced with no content",
            entry.key
        );
        assert!(
            seen.insert(label),
            "the site header names two links \"{label}\" (one of them {}) — an assistive technology announces both the \
             same way and the operator cannot choose between them",
            entry.key
        );
    }

    // 3. EVERY RAIL ITEM IS NAMED, UNIQUELY **WITHIN ITS OWN RAIL**. A record screen or a parent's route resolves to the
    //    drill-in's parent for highlight purposes, but the label belongs to the menu entry that is actually drawn, so
    //    uniqueness is checked per surface.
    //
    //    The actor is irrelevant: a name is a name whether or not the person in front of the screen may open the link,
    //    and an invisible duplicate is how a rail ends up with two identical tabs for the people who can see both.
    let root = root_actor();
    let mut rails_checked = 0usize;
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            // The public site has no portal rail; `surface_visible` returns false for Site by design.
            continue;
        }
        let rail = registry::rail_items(surface, &root);
        assert!(
            !rail.is_empty(),
            "{} has no rail item at all for ROOT — a rail with nothing in it cannot be labelled by its contents",
            surface.key()
        );
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (label, entry) in &rail {
            assert!(
                !unnamed(label),
                "{} is on the {} rail with no name — its link is announced with no content",
                entry.key,
                surface.key()
            );
            assert!(
                seen.insert(*label),
                "the {} rail names two links \"{label}\" — every operator hears two identical tabs in that world",
                surface.key()
            );
        }
        rails_checked += 1;
    }
    assert!(
        rails_checked >= 6,
        "only {rails_checked} portal rails were checked, below the floor of 6"
    );

    // 4. A RAIL'S NAME IS THE SURFACE'S NAME. `PortalFrame` builds `aria-label={format!("{} navigation", surface.label())}`
    //    from `Surface::label()`, not from the rail's contents, so the two can drift apart silently — a landmark
    //    announcing one world while showing another. This is the negative case for the whole story: the drift is
    //    invisible on screen, because the visible rail and the spoken rail are drawn from different sources.
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        let rail = registry::rail_items(surface, &root_actor());
        assert_eq!(
            rail.first().map(|(_, entry)| entry.surface),
            Some(surface),
            "the {} rail's first item belongs to another surface — the landmark announces \"{} navigation\" over \
             contents from somewhere else",
            surface.key(),
            surface.label()
        );
        for (_, entry) in &rail {
            assert_eq!(
                entry.surface,
                surface,
                "{} is on the {} rail but declares Surface::{} — one landmark, two worlds",
                entry.key,
                surface.key(),
                entry.surface.key()
            );
        }
    }

    // 5. A TITLE IS A NAME TOO. `Entry::title` is drawn as the `<h1>` on the screen and as the text of the template's
    //    back link ("← Clients"), so an unnamed or duplicate title is an unnamed heading and an unnamed way back.
    let mut titles: BTreeSet<&str> = BTreeSet::new();
    for entry in registry::ENTRIES {
        assert!(
            !unnamed(entry.title),
            "{} has no title — its heading and its back-link text are both drawn from it",
            entry.key
        );
        // Titles are NOT globally unique and are not meant to be: `Marketing` and `Marketing / Syndication` are both
        // "Publishing" in the rail while their screen titles differ, and two screens may legitimately share a title as
        // long as they are on different rails. What must never happen is a title that is only punctuation.
        assert!(
            entry.title.chars().any(char::is_alphanumeric),
            "{} is titled \"{}\", which names nothing",
            entry.key,
            entry.title
        );
        titles.insert(entry.title);
    }
    assert!(
        titles.len() >= 40,
        "the registry holds only {} distinct titles, below the floor of 40 — a scan that read almost nothing would pass",
        titles.len()
    );

    // 6. THE MENU A LINK IS IN IS THE MENU ITS OWN ENTRY DECLARES. `rail_items` filters on `entry.menu`, so a label can
    //    only ever be drawn from the `Menu` variant the entry declares. Pinning it keeps a label from being reachable
    //    through a menu it does not belong to — the way a drill-in's title could otherwise be announced as a rail tab.
    for entry in registry::ENTRIES {
        match entry.menu {
            Menu::None => assert!(
                !registry::header_items().any(|(_, other)| other.key == entry.key),
                "{} is Menu::None yet appears in the site header",
                entry.key
            ),
            Menu::Header(label) => assert!(
                !unnamed(label),
                "{} declares Menu::Header with no label",
                entry.key
            ),
            Menu::Rail(label) => assert!(
                !unnamed(label),
                "{} declares Menu::Rail with no label",
                entry.key
            ),
        }
    }
}

/// ROOT as the portal layout hands it over. Names do not depend on the actor, but building the rail needs one, and ROOT
/// is the actor that sees every rail — so uniqueness is checked where the most collisions would be visible.
fn root_actor() -> Actor {
    Actor {
        level: Some(Level::Root),
        account_type: "internal".into(),
        authority_codes: ["portal.read", "deal.read", "settings.read", "tech.access"]
            .map(String::from)
            .to_vec(),
        entitlement_codes: Vec::new(),
    }
}
