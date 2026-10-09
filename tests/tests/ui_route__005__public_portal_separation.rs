//! UI.ROUTE — public/portal separation (TST-UI-ROUTE-005).
//!
//! Contract: the area is a fact about the URL, not the menu
//! (`web/ui/src/app/registry.rs:118-123`): every path under `/portal` draws
//! the portal chrome behind the portal's server-side guard, everything else
//! draws the public site — and moving between the two is always a document
//! load, never in-app navigation (`web/ui/src/app/chrome.rs:71-81`, `in_app`).
//! The portal's operating worlds never include the public site
//! (`registry::visible_surfaces` filters `Surface::Site` out unconditionally).
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `registry::ENTRIES`, `Entry::area`, `registry::area_of`,
//! `chrome::in_app` and `registry::visible_surfaces`. Level: L1 Component,
//! harness `MviHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_route__005__public_portal_separation

use ui::app::chrome::in_app;
use ui::app::registry::{area_of, resolve, visible_surfaces, Area, ENTRIES};
use ui::model::Surface;
use ui::navigation::{Actor, Level};

const HARNESS: &str = "MviHarness/L1 Component";

/// ROOT as the portal layout hands it over: the level AND the authorities its
/// roles carry (mirrors the registry's own test actor).
fn root() -> Actor {
    Actor {
        level: Some(Level::Root),
        account_type: "internal".into(),
        authority_codes: ["portal.read", "deal.read", "settings.read", "tech.access"]
            .map(String::from)
            .to_vec(),
        ..Actor::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ROUTE-005); the file and the assay use it.
fn ui_route_005__public_portal_separation() {
    // 1. The area rule is exhaustive and exact: a registered entry is Portal
    //    iff its path is `/portal` or below it. No portal screen is filed
    //    under a public path, and no public page is filed under `/portal` —
    //    so the guard and the chrome can never disagree about a known route.
    for entry in ENTRIES {
        let under_portal = entry.path == "/portal" || entry.path.starts_with("/portal/");
        assert_eq!(
            entry.area(),
            area_of(entry.path),
            "{HARNESS}: Entry::area must agree with area_of for '{}'",
            entry.key
        );
        assert_eq!(
            entry.area() == Area::Portal,
            under_portal,
            "{HARNESS}: '{}' at '{}' is on the wrong side of the portal boundary",
            entry.key,
            entry.path
        );
    }

    // Both worlds are actually populated, so the loop above is not vacuous.
    assert!(
        ENTRIES.iter().any(|entry| entry.area() == Area::Portal),
        "{HARNESS}: the table carries portal entries"
    );
    assert!(
        ENTRIES.iter().any(|entry| entry.area() == Area::Site),
        "{HARNESS}: the table carries public site entries"
    );

    // 2. Resolution never crosses the boundary: a portal path resolves to a
    //    portal entry, a site path to a site entry. A portal URL can never
    //    render a public page's screen and vice versa.
    for entry in ENTRIES {
        // Skip parametric shapes already proven elsewhere; resolve the entry's
        // own concrete path shape via a representative static check below.
        if entry.path.contains(':') {
            continue;
        }
        let (here, _) = resolve(entry.path)
            .unwrap_or_else(|| panic!("{HARNESS}: entry '{}' must resolve", entry.key));
        assert_eq!(
            here.area(),
            entry.area(),
            "{HARNESS}: '{}' resolved across the portal boundary",
            entry.key
        );
    }
    // Parametric crossings are closed too: record/detail shapes stay portal.
    let (client_record, _) = resolve("/portal/clients/abc").expect("client record resolves");
    assert_eq!(client_record.area(), Area::Portal);
    let (property_detail, _) =
        resolve("/properties/casa-luar").expect("public property detail resolves");
    assert_eq!(property_detail.area(), Area::Site);

    // 3. Crossing the boundary is never in-app, in either direction: the
    //    portal's actor snapshot only exists on a portal page, so the move
    //    must load the document.
    assert!(
        !in_app("/portal/dashboard", "/buyers"),
        "{HARNESS}: portal to site must load the document"
    );
    assert!(
        !in_app("/buyers", "/portal/dashboard"),
        "{HARNESS}: site to portal must load the document"
    );
    assert!(
        !in_app("/portal/clients", "/login"),
        "{HARNESS}: portal to login must load the document"
    );
    assert!(
        !in_app("/login", "/portal/clients"),
        "{HARNESS}: login to portal must load the document"
    );
    assert!(
        !in_app("/portal/dashboard", "/"),
        "{HARNESS}: portal to site home must load the document"
    );
    assert!(
        !in_app("/", "/portal/dashboard"),
        "{HARNESS}: site home to portal must load the document"
    );
    // The external islands obey the same boundary: the portal-root island to
    // the site home still loads the document.
    assert!(
        !in_app("/portal", "/"),
        "{HARNESS}: even the portal-root island leaves via a document load"
    );

    // 4. POSITIVE CONTROLS — the refusals above are the boundary, not a
    //    broken rule: same-area, non-external moves stay in-app on both sides.
    assert!(
        in_app("/portal/dashboard", "/portal/deals"),
        "{HARNESS}: portal to portal stays in-app"
    );
    assert!(
        in_app("/buyers", "/about"),
        "{HARNESS}: site to site stays in-app"
    );

    // 5. The operating worlds are portal-only: the public site is never a
    //    world ROOT can open, and an external account opens none at all — so
    //    no menu can offer the site as a portal surface.
    let worlds = visible_surfaces(&root());
    assert!(
        !worlds.contains(&Surface::Site),
        "{HARNESS}: the public site is never an operating world, got {worlds:?}"
    );
    assert_eq!(
        worlds.len(),
        6,
        "{HARNESS}: ROOT opens the six portal worlds, got {worlds:?}"
    );
    let guest = Actor {
        account_type: "external".into(),
        ..root()
    };
    assert!(
        visible_surfaces(&guest).is_empty(),
        "{HARNESS}: an external account opens no operating world"
    );

    // 6. NEGATIVE — the boundary holds for paths no screen serves: a 404 is
    //    still drawn in the right chrome, and it is still never in-app.
    assert_eq!(area_of("/portal/no-such-screen"), Area::Portal);
    assert_eq!(area_of("/no-such-page"), Area::Site);
    assert!(
        !in_app("/portal/dashboard", "/portal/no-such-screen"),
        "{HARNESS}: an unknown portal path is never an in-app destination"
    );
    assert!(
        in_app("/portal/no-such-screen", "/portal/dashboard"),
        "{HARNESS}: an unknown portal source moves in-app to a portal screen — same area, no boundary crossed"
    );
    assert!(
        !in_app("/portal/no-such-screen", "/buyers"),
        "{HARNESS}: an unknown portal source still cannot cross to the site in-app"
    );
}
