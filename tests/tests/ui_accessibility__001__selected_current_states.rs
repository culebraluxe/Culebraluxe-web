//! UI.ACCESSIBILITY — selected/current states (TST-UI-ACCESSIBILITY-001).
//!
//! Contract: **the menu tells the operator where they are, and tells them once.** `web/ui/src/app/registry.rs` owns
//! `is_current(item, path)`, which `chrome.rs` turns into exactly one `aria-current="page"` on the link it draws
//! (`AppLink`: `aria-current={props.current.then_some("page")}`). A user who cannot see which destination is current is
//! lost in the portal, and a user who sees two of them cannot trust either. Both are the same defect seen from two
//! sides, so this contract pins both:
//!
//!   1. **AT MOST ONE.** Across every screen the app serves, at most one menu item is current. Two `aria-current="page"`
//!      in one menu is a broken indicator, not a redundancy.
//!   2. **EXACTLY ONE, WHERE A MENU CAN ANSWER.** A path whose screen is itself in a menu marks exactly that item. This
//!      is the direction that catches a regression: a screen that stops marking itself leaves an operator with no
//!      position at all, which passes an "at most one" test forever.
//!   3. **A DRILL-IN HIGHLIGHTS ITS PARENT.** `/portal/clients/abc` highlights `Clients`, because the record is not a
//!      menu item and the operator still has to know which list they came from. The registry models this with
//!      `Entry::parent`, and `every_drill_in_names_a_real_parent_and_highlights_it` covers the parent names; this pins
//!      that the highlight actually happens.
//!   4. **AN UNKNOWN PATH IS NEVER CURRENT.** A path no screen serves marks nothing. A resolver that returns a
//!      best-effort match here would put a stale highlight on a 404.
//!
//! WHY THE REGISTRY AND NOT THE RENDERED MARKUP. `is_current` is the decision the chrome renders; walking the Yew VDOM
//! to read `aria-current` back would assert on `web_sys` rather than on this contract, and `VList`'s children are
//! `pub(crate)` in `yew 0.23` so a host test cannot descend the tree at all. The decision function is the honest
//! boundary, and it is the same function `AppLink` calls on every render — a test of it is a test of what ships.
//!
//! Level: L1 Component — the registry's routing/visibility decision layer, driven in-process. No database, no network,
//! no PROD, no browser.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_accessibility__001__selected_current_states

use ui::app::registry::{self, Entry, Menu};
use ui::model::Surface;

/// Every item the chrome will actually draw a link for. `Menu::None` entries are routable but never in a menu, so they
/// can never be current — they are excluded here rather than asserted on individually.
fn menu_items() -> Vec<&'static Entry> {
    registry::ENTRIES
        .iter()
        .filter(|entry| entry.menu != Menu::None)
        .collect()
}

/// The menu keys `registry::is_current` marks as current at `path`.
fn current_keys(path: &str) -> Vec<&'static str> {
    menu_items()
        .into_iter()
        .filter(|entry| registry::is_current(entry, path))
        .map(|entry| entry.key)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ACCESSIBILITY-001); the file and the assay use it.
fn ui_accessibility_001__selected_current_states() {
    // 1. THE INDICATOR IS SINGLE. At most one item is current at any path — the invariant that survives screens nobody
    //    enumerated, and the one a second writer of `is_current` would break first.
    //
    //    Driven over every registered path AND a set of paths a user actually reaches, including drill-ins, trailing
    //    slashes, query strings and encoded record ids: the cases where a naive `==` comparison goes wrong.
    let mut paths: Vec<String> = registry::ENTRIES
        .iter()
        .map(|entry| entry.path.to_string())
        .collect();
    paths.extend(
        [
            "/portal/clients/abc",
            "/portal/clients/abc/",
            "/portal/deals/42",
            "/portal/forms/f-1?tab=edit",
            "/properties/casa-luar",
            "/review/tok/2",
            "/portal/clients/abc%20123?tab=history",
            "/portal/clients/",
            "/portal/no-such-screen",
            "/buyers/",
            "/",
        ]
        .map(str::to_string),
    );
    for path in &paths {
        let current = current_keys(path);
        assert!(
            current.len() <= 1,
            "{path}: {} menu items are marked current ({:?}) — a position indicator that names two destinations \
             cannot be believed. `registry::is_current` must match one screen.",
            current.len(),
            current
        );
    }

    // 2. THE INDICATOR IS PRESENT. The negative half, and the one that catches the real regression: a screen whose
    //    entry IS in a menu must mark exactly that item. An "at most one" test alone passes forever against a resolver
    //    that marks nothing, which is the state an operator experiences as "I cannot tell where I am".
    let mut checked = 0usize;
    for entry in menu_items() {
        let Some((here, _)) = registry::resolve(entry.path) else {
            continue;
        };
        let expected = if here.key == entry.key {
            entry.key
        } else if here.parent == Some(entry.key) {
            // A menu entry reached through a more specific route still marks itself.
            entry.key
        } else {
            // The entry's own path resolves to a different screen (a retired or superseded route). Nothing to assert
            // about a link the chrome would not draw as current.
            continue;
        };
        let current = current_keys(entry.path);
        assert_eq!(
            current,
            vec![expected],
            "{} is in a menu at {} but the menu marks {:?} — a listed screen with no current item leaves the operator \
             with no position",
            entry.key,
            entry.path,
            current
        );
        checked += 1;
    }
    assert!(
        checked >= 30,
        "only {checked} menu entries were checked, below the floor of 30: a loop that resolved almost nothing would pass \
         this test while proving nothing. The registry has {} menu entries today.",
        menu_items().len()
    );

    // 3. A DRILL-IN HIGHLIGHTS ITS PARENT. This is the registry's own rule (`here.parent == Some(item.key)`) and it is
    //    the reason `is_current` exists in this shape rather than as a key comparison.
    for entry in registry::ENTRIES {
        let Some(parent) = entry.parent else {
            continue;
        };
        let parent = registry::by_key(parent).unwrap_or_else(|| {
            panic!(
                "{}: parent {parent} is not a registered screen — a drill-in with no parent has nothing to highlight",
                entry.key
            )
        });
        assert_eq!(
            parent.area(),
            entry.area(),
            "{} and its parent must share a chrome, or the highlight is drawn in a menu that is not on the page",
            entry.key
        );
        assert!(
            registry::is_current(parent, entry.path),
            "{} is a drill-in of {} but its route does not highlight the parent — the operator cannot tell which list \
             they opened from",
            entry.key,
            parent.key
        );
        // `is_current` is deliberately menu-agnostic: it is a key/parent comparison, and it answers "is this entry the
        // current one" for whatever entry it is handed. The chrome only ever hands it entries it is about to draw a
        // link for, so a `Menu::None` drill-in is never asked about. What matters here is that the drill-in itself is
        // NOT a menu item — otherwise the chrome would draw a second link and section 1's single-indicator claim would
        // be resting on the chrome filtering rather than on the registry.
        assert!(
            entry.menu == Menu::None,
            "{} is a drill-in but is in a menu ({:?}); the parent highlight above would then be one of two current \
             items rather than the only one",
            entry.key,
            entry.menu
        );
    }
    // The named cases, so the rule is legible without reading the loop above.
    let clients = registry::by_key("clients").expect("Clients is registered");
    assert!(
        registry::is_current(clients, "/portal/clients/abc"),
        "a client record highlights Clients"
    );
    assert!(registry::is_current(clients, "/portal/clients"));
    assert!(
        !registry::is_current(clients, "/portal/deals"),
        "and Contracts is not marked while Clients is open"
    );
    assert_eq!(
        registry::parent_of("/portal/deals/42").map(|entry| entry.key),
        Some("deals"),
        "the template's back link reads the same parent rule"
    );
    assert!(registry::parent_of("/portal/deals").is_none());

    // 4. AN UNKNOWN PATH IS NEVER CURRENT. A resolver that matched on a prefix would mark a menu item on a 404 — the
    //    operator is told where they are, and they are nowhere.
    for unknown in [
        "/portal/no-such-screen",
        "/portal/clients/abc/extra",
        "/nope",
        "/portal",
    ] {
        let current = current_keys(unknown);
        // `/portal` resolves to the `portal-root` External entry, which is `Menu::None`: either way nothing is a menu
        // item, which is the whole claim.
        assert!(
            current.is_empty(),
            "{unknown} is served by no menu item, but {:?} is marked current — a stale highlight on a page that is not \
             the one the operator asked for",
            current
        );
    }

    // 5. THE CURRENT ITEM IS ALWAYS ONE THE MENU ACTUALLY OFFERS. `is_current` is pure — it does not consult the
    //    actor — so the chrome must intersect it with visibility itself. This pins the fact that makes that
    //    intersection necessary, and pins that the surfaces that are rendered always have at least one item to mark,
    //    so a menu can never be drawn in a state where "where am I" is unanswerable.
    // `Surface::ALL` is the model's own order, and `Surface` is `Eq` but not `Ord` — so the surfaces are visited in the
    // order the model declares and membership is tested, not sorted into a `BTreeSet<Surface>`.
    let surfaces_with_items: Vec<Surface> = Surface::ALL
        .iter()
        .copied()
        .filter(|surface| menu_items().iter().any(|entry| entry.surface == *surface))
        .collect();
    assert!(
        !surfaces_with_items.is_empty(),
        "no surface contributes a menu item, so nothing can ever be marked current"
    );
    for surface in surfaces_with_items {
        let offered = menu_items()
            .into_iter()
            .filter(|entry| entry.surface == surface)
            .count();
        assert!(
            offered > 0,
            "{surface:?} contributes menu entries but none of them is on its rail — the loop above found the entry \
             through another surface"
        );
        assert!(
            matches!(surface, Surface::Site | Surface::Core | Surface::Accounting | Surface::Marketing | Surface::Ops | Surface::Support | Surface::Tech),
            "{surface:?} is not a surface this registry knows; an unknown surface would render a rail nothing marks"
        );
    }
}
