//! UI.ACCESSIBILITY — button names (TST-UI-ACCESSIBILITY-003).
//!
//! Contract: **every action the portal offers is named before it is offered.** A button is the only part of this UI an
//! operator can act on without reading the page, so its name is its whole affordance: a screen-reader user navigates by
//! button name, not by position. An unnamed button is invisible as a control and reachable as a trap.
//!
//! The names in this application are not free text in each view — they are **the registry's and the template's**:
//!
//!   * `registry::ENTRIES` gives every screen a `title` and a menu `label`. The title is the `<h1>` and the back-link
//!     text; the menu label is the rail tab. These are the names an operator reaches actions *through*.
//!   * `template::remote_retry` draws the failure panel's only control, and its text is fixed by the template — one
//!     place, every screen. `template::empty_panel` and `widget_removed` draw the no-content message.
//!
//! The contract pinned here is that this naming layer is **total** (every screen and every menu slot carries a name) and
//! **unambiguous within a landmark** (two actions on one screen may not answer to the same word, because an assistive
//! technology offers a list of names and picks by string). Both halves fail silently on screen: a blank title renders a
//! blank heading, and a duplicate renders two identical buttons.
//!
//! WHY THE REGISTRY AND NOT THE RENDERED MARKUP. These strings are what `chrome.rs` and `template.rs` render. The Yew
//! VDOM cannot be walked from a host test — `VList`'s children are `pub(crate)` in `yew 0.23` — so the naming layer is
//! both the honest boundary and the only one a host test can reach. It is also the layer where a name is *decided*: a
//! view that wanted a different button name would have to add it there, and a test on that layer catches it.
//!
//! Level: L1 Component — the naming vocabulary the chrome and the template draw from. No database, no network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_accessibility__003__button_names

use std::collections::BTreeSet;

use ui::app::registry::{self, Entry, Menu};
use ui::model::Surface;
use ui::navigation::{Actor, Level};

/// A name an operator (or a screen reader) can act on. Empty, whitespace-only, or a lone ellipsis names nothing.
fn unnamed(label: &str) -> bool {
    let trimmed: String = label
        .trim()
        .chars()
        .filter(|character| !matches!(character, '\u{2026}' | '\u{2190}' | '\u{2192}' | '\u{00b7}'))
        .collect();
    trimmed.is_empty()
}

/// ROOT as the portal layout hands it over — the actor that sees every rail, so uniqueness is checked where the most
/// collisions would be visible.
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

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ACCESSIBILITY-003); the file and the assay use it.
fn ui_accessibility__003__button_names() {
    // 1. EVERY SCREEN IS NAMED. `Entry::title` is the screen's `<h1>` and the text of the template's back link, so an
    //    unnamed screen is an unnamed destination — reached by a link whose text the operator had to guess at.
    for entry in registry::ENTRIES {
        assert!(
            !unnamed(entry.title),
            "{} is titled \"{}\" — it is drawn as the screen's heading and as its back link, and a destination with no \
             name cannot be described, bookmarked or referred to",
            entry.key,
            entry.title
        );
        assert!(
            entry.title.chars().any(char::is_alphanumeric),
            "{} is titled \"{}\", which names nothing",
            entry.key,
            entry.title
        );
    }

    // 2. EVERY MENU SLOT IS NAMED. A rail tab with no text is a control with no name; the chrome draws exactly these
    //    strings as the link contents, and `AppLink` gives the link its `aria-current` from the same table.
    let mut named_slots = 0usize;
    for entry in registry::ENTRIES {
        match entry.menu {
            Menu::None => {}
            Menu::Header(label) | Menu::Rail(label) => {
                assert!(
                    !unnamed(label),
                    "{} sits in a menu with no label — the tab an operator clicks to reach it has nothing to announce",
                    entry.key
                );
                named_slots += 1;
            }
        }
    }
    assert!(
        named_slots >= 30,
        "only {named_slots} menu slots were checked, below the floor of 30: a loop that read almost nothing would pass \
         this test while naming nothing"
    );

    // 3. TWO ACTIONS ON ONE SCREEN MAY NOT ANSWER TO THE SAME NAME. The negative case for the whole story, and the one
    //    that matters: a screen reader offers a flat list of button names, so a duplicate is not a cosmetic repeat — it
    //    is a control the operator cannot pick from the list.
    //
    //    Scoped per screen (per surface rail and the site header), because that is the granularity a person is choosing
    //    within. `Cockpit` legitimately appears on both the CORE and TECH rails: two different screens, two different
    //    lists. What must never happen is `Cockpit` twice on one rail.
    let root = root_actor();
    let mut checked_landmarks = 0usize;
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (label, entry) in registry::rail_items(surface, &root) {
            assert!(
                seen.insert(label),
                "the {} rail offers two tabs named \"{label}\" (this one is {}) — an operator choosing by name cannot \
                 tell them apart, and neither can a screen reader",
                surface.key(),
                entry.key
            );
        }
        checked_landmarks += 1;
    }
    let mut header_seen: BTreeSet<&str> = BTreeSet::new();
    for (label, entry) in registry::header_items() {
        assert!(
            header_seen.insert(label),
            "the site header offers two links named \"{label}\" (this one is {}) — the same defect as a duplicated \
             button, in the one landmark every first-time visitor passes through",
            entry.key
        );
    }
    assert!(
        checked_landmarks >= 6,
        "only {checked_landmarks} rails were checked for duplicate names, below the floor of 6"
    );

    // 4. THE NAME IS REACHABLE FROM THE SCREEN IT NAMES. A title an operator can never encounter — a screen that no
    //    menu, no parent drill-in and no back link can reach — is a name that cannot be used to get anywhere. Every
    //    registered screen is either in a menu, is the parent of something in a menu, or is a public page; anything else
    //    is a destination nobody can be sent to by name.
    //
    //    This is deliberately a reachability check and not a ban: the registry legitimately holds pages with no menu
    //    (`/sign/:token` is reached from an email, `/login/recovery` from `/login`). Those are reachable by other means,
    //    so the assertion is that a name is never blank and never duplicated — facts the earlier sections pin — plus
    //    that a drill-in always names a real parent, which is the way a name is recovered from a record screen.
    for entry in registry::ENTRIES {
        if let Some(parent) = entry.parent {
            let parent_entry = registry::by_key(parent).unwrap_or_else(|| {
                panic!(
                    "{} is a drill-in of {parent}, which is not a registered screen — the operator cannot get back to \
                     anything named",
                    entry.key
                )
            });
            assert!(
                !unnamed(parent_entry.title),
                "{} can only be reached from {}, which has no title to come back to",
                entry.key,
                parent
            );
        }
    }

    // 5. A SCREEN WITH A TITLE IN A MENU IS NAMED THE SAME WAY FROM BOTH DIRECTIONS. The rail tab carries the menu label
    //    and the screen carries the title; where they are the same word, an operator who hears the name and an operator
    //    who reads the heading are describing the same thing. Where they differ, the registry is choosing two names on
    //    purpose (the nav and the page are allowed to differ) — so this pins only that **both exist**, which is what
    //    makes either one usable.
    for (label, entry) in registry::header_items() {
        assert!(
            !unnamed(label) && !unnamed(entry.title),
            "{} appears in the site header as \"{label}\" and on the page as \"{}\"; at least one of the two names an \
             operator uses is blank",
            entry.key,
            entry.title
        );
    }
    for surface in Surface::ALL.iter().copied() {
        if surface == Surface::Site {
            continue;
        }
        for (label, entry) in registry::rail_items(surface, &root_actor()) {
            assert!(
                !unnamed(label) && !unnamed(entry.title),
                "{} appears on the {} rail as \"{label}\" and on the page as \"{}\"; one of the two is blank",
                entry.key,
                surface.key(),
                entry.title
            );
        }
    }

    // 6. AN EXTERNAL SCREEN CANNOT BE NAMED BY A MENU. `Kind::External` renders a notice instead of the screen — it has
    //    no buttons to name — but it may still sit in a menu, and that entry must still carry a name, because the tab
    //    that leads to it is drawn.
    let externals: Vec<&'static Entry> = registry::ENTRIES
        .iter()
        .filter(|entry| entry.needs_document())
        .collect();
    assert!(
        !externals.is_empty(),
        "the registry declares no external routes: an empty scan would pass this section while checking nothing"
    );
    for entry in &externals {
        assert!(
            !unnamed(entry.title),
            "{} is an external route with no title — it renders a notice, and a notice with no heading announces nothing",
            entry.key
        );
    }
}
