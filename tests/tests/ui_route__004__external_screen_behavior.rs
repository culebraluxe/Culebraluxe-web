//! UI.ROUTE — external screen behavior (TST-UI-ROUTE-004).
//!
//! Contract: a route this app cannot render yet (`Kind::External` in
//! `web/ui/src/app/registry.rs`) still resolves, announces that it needs the
//! document (`Entry::needs_document`), and is never treated as in-app
//! navigation — the shell shows its notice panel instead of reloading the
//! document (which would land right back in this app;
//! `web/ui/src/app/shell.rs:114-120`), and the chrome link rule
//! (`web/ui/src/app/chrome.rs:71-81`, `in_app`) forces a document load both
//! *from* and *to* an external entry.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `registry::ENTRIES`, `registry::resolve`, `Entry::needs_document`
//! and `chrome::in_app`. Level: L1 Component, harness `MviHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_route__004__external_screen_behavior

use ui::app::chrome::in_app;
use ui::app::registry::{resolve, by_key, ENTRIES, Kind};

const HARNESS: &str = "MviHarness/L1 Component";
/// The portal island an external entry can never navigate within.
const PORTAL_SCREEN: &str = "/portal/dashboard";
/// A portal screen link, for the positive in-app control.
const PORTAL_OTHER: &str = "/portal/deals";

/// The entries production marks external: a route the Rust app cannot render
/// yet. The shell renders each as a notice panel, never a document reload.
fn externals() -> Vec<&'static ui::app::registry::Entry> {
    ENTRIES
        .iter()
        .filter(|entry| matches!(entry.kind, Kind::External))
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ROUTE-004); the file and the assay use it.
fn ui_route_004__external_screen_behavior() {
    // 1. The external islands resolve by path, with no params to hand a screen.
    let (coexistence, coexistence_params) = resolve("/portal/admin/whatsapp-coexistence")
        .expect("the WhatsApp Activation external resolves");
    assert_eq!(
        coexistence.key, "whatsapp-coexistence",
        "{HARNESS}: the coexistence path resolves to its own entry"
    );
    assert!(
        coexistence_params.is_empty(),
        "{HARNESS}: an external entry carries no screen params"
    );
    let (portal_root, root_params) =
        resolve("/portal").expect("the portal root external resolves");
    assert_eq!(
        portal_root.key, "portal-root",
        "{HARNESS}: /portal resolves to its own entry"
    );
    assert!(
        root_params.is_empty(),
        "{HARNESS}: the portal root carries no screen params"
    );
    assert!(
        matches!(coexistence.kind, Kind::External)
            && matches!(portal_root.kind, Kind::External),
        "{HARNESS}: both resolve as External, never as a Screen mount"
    );

    // 2. `needs_document` is exactly the external flag: true for every
    //    External entry, false for every Screen entry. The shell branches on
    //    this (notice panel vs. mount), so a Screen that claimed the document
    //    would unload the app, and an External that did not would be mounted
    //    as a screen it is not.
    for entry in ENTRIES {
        let expected = matches!(entry.kind, Kind::External);
        assert_eq!(
            entry.needs_document(),
            expected,
            "{HARNESS}: needs_document must equal the External kind for '{}'",
            entry.key
        );
    }
    assert!(
        !externals().is_empty(),
        "{HARNESS}: the table actually carries external entries, so the loop above is not vacuous"
    );

    // 3. Leaving an external island is never in-app — even to a same-area
    //    screen. The notice panel has no in-app destination; following any
    //    link from it loads the document.
    for external in externals() {
        assert!(
            !in_app(external.path, PORTAL_SCREEN),
            "{HARNESS}: leaving external '{}' must load the document, even to {}",
            external.key, PORTAL_SCREEN
        );
        assert!(
            !in_app(external.path, external.path),
            "{HARNESS}: an external entry is not even in-app with itself"
        );
    }

    // 4. Arriving at an external island is never in-app either — even from a
    //    same-area screen. The rail offers WhatsApp Activation as a menu link;
    //    following it must load the document rather than move inside the app.
    for external in externals() {
        assert!(
            !in_app(PORTAL_SCREEN, external.path),
            "{HARNESS}: entering external '{}' from a portal screen must load the document",
            external.key
        );
    }

    // 5. POSITIVE CONTROL — the refusals above are about externals, not a
    //    broken rule: two non-external same-area screens navigate in-app.
    assert!(
        in_app(PORTAL_SCREEN, PORTAL_OTHER),
        "{HARNESS}: portal screen to portal screen stays in-app"
    );
    assert!(
        in_app(PORTAL_OTHER, PORTAL_SCREEN),
        "{HARNESS}: the rule is symmetric for non-external entries"
    );

    // 6. Externals keep their chrome: only the signing page is chrome-free,
    //    so an external notice still renders inside the portal frame.
    assert_eq!(
        by_key("sign-document").map(|entry| entry.chrome_free()),
        Some(true),
        "{HARNESS}: the chrome-free control exists"
    );
    for external in externals() {
        assert!(
            !external.chrome_free(),
            "{HARNESS}: external '{}' renders in chrome, like every non-signing entry",
            external.key
        );
    }

    // 7. NEGATIVE — an unregistered path resolves to nothing and is never an
    //    in-app destination, so a typo cannot be mistaken for an island.
    assert!(
        resolve("/portal/admin/no-such-island").is_none(),
        "{HARNESS}: an unknown portal path must not resolve"
    );
    assert!(
        !in_app(PORTAL_SCREEN, "/portal/admin/no-such-island"),
        "{HARNESS}: an unknown path is never an in-app destination"
    );
    // A bare external-looking prefix with no entry is not an island either.
    assert!(
        resolve("/portal/admin").is_none(),
        "{HARNESS}: a path prefix without an entry must not resolve"
    );
}
