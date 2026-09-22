//! The Rust UI, built on MVI: Model → View → Intent, one direction round.
//!
//! WHY MVI: the pattern makes a screen a pure function of data. `Model` is the whole screen state, `Msg` is
//! everything that can happen to it, `update` is the only thing that changes it and it is pure, and `view` renders
//! the model without deciding anything. The interaction model is therefore testable without a browser, and a UI bug
//! is a reducer bug with a named message instead of a mystery in a lifecycle hook.
//!
//! THE RULES THIS CRATE KEEPS, matching the Rust port's other boundaries:
//!
//! 1. **One owner of application state.** The Rust model owns what is selected, what is loading, and what the server
//!    said. A third-party widget owns only its own rendering and local mechanics, receives a snapshot, and reports
//!    intents back — never a competing model, never its own save.
//! 2. **Strict DOM ownership.** Rust owns a widget's container; the widget owns everything inside it. Neither edits
//!    the other's DOM. (Enforced by the shell, not by this crate.)
//! 3. **A small typed bridge.** Plain data and named events, keyed by stable ids.
//! 4. **The server stays authoritative.** This crate holds no credential and decides no permission. It calls the
//!    application's own routes, which own the session and the server-side credentials; a rejected change arrives as
//!    `EffectFailed` and restores the model, which is how a widget snaps back.
//!
//! SCOPE: the portal menu. `SCREENS` is the port's to-do list, `Screen::path` names the live route each
//! entry replaces, and Project Management is deliberately a placeholder.

pub mod document;
pub mod model;
pub mod update;
pub mod view;

#[cfg(feature = "wasm")]
pub mod shell;

/// The Yew application, which is replacing `shell` for the public routes. Everything it needs of the MVI core
/// (`model`, `update`, `view`) is shared with the string renderers, so both runtimes agree on state and on the search
/// contract while the port completes.
#[cfg(all(feature = "wasm", feature = "yew"))]
pub mod yew_app;
#[cfg(all(feature = "wasm", feature = "yew"))]
pub mod yew_effects;
#[cfg(all(feature = "wasm", feature = "yew-router"))]
pub mod yew_router;
#[cfg(all(feature = "wasm", feature = "yew"))]
pub mod yew_portal;
#[cfg(all(feature = "wasm", feature = "yew"))]
pub mod yew_views;

pub mod icons;
pub use document::{document, ASSETS_ROOT, STYLESHEET_URL};
pub use model::{
    home, listed, record_for, screen, Block, BlockItem, Controls, Effect, Listing, Model, Msg, Nav, PageContent, Row,
    Screen, Surface, PAGE_SIZE, SCREENS,
};
pub use update::update;
pub use view::{render, render_page, PAGE_ID};

/// The whole program, so a host never has to remember the loop's shape.
#[derive(Debug, Default)]
pub struct Program {
    model: Model,
}

impl Program {
    pub fn new() -> Self {
        Self {
            model: Model::default(),
        }
    }

    /// Start on a specific screen. The host knows the URL and therefore knows what the user asked for; the model does
    /// not guess.
    pub fn open(&mut self, screen: Screen) -> Vec<Effect> {
        self.dispatch(Msg::ScreenOpened(screen))
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Apply one intent, returning the effects the host has to perform. The model is already updated when this
    /// returns, so the view can be re-rendered immediately — a request is a consequence of the state change, never a
    /// precondition for it.
    pub fn dispatch(&mut self, msg: Msg) -> Vec<Effect> {
        update(&mut self.model, msg)
    }

    pub fn html(&self) -> String {
        render(&self.model)
    }

    /// The screen's own area — everything inside `#rust-page` and nothing else.
    ///
    /// The shell repaints this for a screen-local message and the whole document only when the chrome itself changed.
    /// Both halves come from here rather than from the shell, because which markup is chrome is a property of the view.
    pub fn page_html(&self) -> String {
        view::render_page(&self.model)
    }

    /// What the chrome is a function of: the screen being shown, and its surface.
    ///
    /// The shell compares this before and after a message and repaints the chrome only when it differs. A signature
    /// rather than a list of "messages that change the screen" because such a list is a second place to be wrong the
    /// moment a new message can navigate — and comparing the thing itself is what keeps the rule true without anyone
    /// maintaining it.
    pub fn chrome_signature(&self) -> String {
        format!("{:?}:{}", self.model.screen.surface, self.model.screen.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Screens are addressed by KEY in these tests: the table is the source of truth, so a test that named a variant
    /// would be asserting a name that only exists in a previous version of this file.
    fn target(key: &str) -> Screen {
        crate::model::screen(key).expect("a screen the table defines")
    }

    #[test]
    fn the_effect_wire_format_is_exactly_what_the_host_looks_for() {
        // THE CONTRACT THAT WAS BROKEN FOR EVERY RELEASE OF THIS UI. The host switches on the tag string
        // (`FetchRows`, `FetchPage`); when this enum renamed its tags to camelCase, every effect was emitted with a
        // name the host did not recognise, and an unrecognised effect is ignored in silence — no data, no error, just
        // the chrome. Nothing tested the wire format, so nothing caught it. This does.
        assert_eq!(
            serde_json::to_string(&Effect::FetchRows {
                screen: "clients",
                scope: None,
                generation: 4
            })
            .unwrap(),
            r#"{"effect":"FetchRows","screen":"clients","scope":null,"generation":4}"#
        );
        assert_eq!(
            serde_json::to_string(&Effect::FetchPage {
                screen: "site-home",
                scope: None,
                generation: 0
            })
            .unwrap(),
            r#"{"effect":"FetchPage","screen":"site-home","scope":null,"generation":0}"#
        );
        // A page about one record names it, in the same request that names the screen. The child page depended on this
        // and did not get it: the effect carried no key, so the host could not tell the page route which property to
        // serve and the route — which refuses to guess — answered with an error.
        assert_eq!(
            serde_json::to_string(&Effect::FetchPage {
                screen: "site-property-detail",
                scope: Some("villa-rosada".into()),
                generation: 12
            })
            .unwrap(),
            r#"{"effect":"FetchPage","screen":"site-property-detail","scope":"villa-rosada","generation":12}"#
        );
    }

    #[test]
    fn a_new_program_opens_on_the_first_menu_screen_with_nothing_loaded() {
        let program = Program::new();
        assert_eq!(program.model().screen, SCREENS[0]);
        assert!(program.model().rows.is_empty());
        assert!(!program.model().loading);
    }

    #[test]
    fn opening_a_screen_asks_the_host_for_its_rows() {
        let mut program = Program::new();
        let effects = program.dispatch(Msg::ScreenOpened(target("clients")));
        assert!(
            program.model().loading,
            "the model must reflect the intent immediately"
        );
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "clients",
                scope: None,
                generation: 0,
            }]
        );
    }

    #[test]
    fn the_view_renders_the_model_it_was_given() {
        let mut program = Program::new();
        // A mount, so the model's generation matches the response's: opening a screen IS the mount here (this is the
        // shape the shell uses), and a response is only applied by the run that asked for it.
        program.dispatch(Msg::Mount {
            screen: target("clients"),
            generation: 1,
        });
        let before = program.html();
        program.dispatch(Msg::RowsLoaded {
            screen: "clients".into(),
            generation: 1,
            rows: vec![Row {
                id: "c1".into(),
                cells: vec!["Ada Lovelace".into(), "Buyer".into()],
                badge: Some("new".into()),
            }],
        });
        let after = program.html();
        assert_ne!(
            before, after,
            "state that does not change the view is state nothing needed"
        );
        assert!(after.contains("Ada Lovelace"));
    }

    /// THE INVARIANT THE BROWSER BUG BROKE: **a response issued for screen A can never mutate screen B.**
    ///
    /// Two requests are in flight, the visitor clicks away, and the slow one lands second. Arrival order says nothing
    /// about ownership, so the response has to say whose it is — and this is that, tested where it can be tested without
    /// a browser. The generation half is the same rule for a screen the visitor has left and come back to.
    #[test]
    fn a_response_for_another_screen_or_another_mount_is_discarded() {
        let mut program = Program::new();
        program.dispatch(Msg::Mount {
            screen: target("site-home"),
            generation: 7,
        });
        let home = program.model().page.clone();

        // A page fetched for Buyers, under the mount that asked for it, arriving while Home is on screen.
        program.dispatch(Msg::PageLoaded {
            screen: "site-buyers".into(),
            generation: 7,
            page: crate::model::PageContent {
                hero: crate::model::Block {
                    title: "Buyers hero".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        });
        assert_eq!(
            program.model().page, home,
            "a payload for another screen must not become the page on screen"
        );
        assert!(
            !program.html().contains("Buyers hero"),
            "and it must not reach the view"
        );

        // And the same screen under a mount that has been replaced: the screen matches, the mount does not.
        program.dispatch(Msg::PageLoaded {
            screen: "site-home".into(),
            generation: 6,
            page: crate::model::PageContent {
                hero: crate::model::Block {
                    title: "A stale mount".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        });
        assert!(
            !program.html().contains("A stale mount"),
            "a payload from a previous mount of this screen is not this mount's answer"
        );

        // The current mount's own response IS applied, so the guard refuses the stale and not the wanted.
        program.dispatch(Msg::PageLoaded {
            screen: "site-home".into(),
            generation: 7,
            page: crate::model::PageContent {
                hero: crate::model::Block {
                    title: "The home hero".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        });
        assert!(program.html().contains("The home hero"));
        assert!(!program.model().loading, "and the screen is done loading");
    }

    /// A mount stamps its generation on the requests it makes, which is what lets the answer be matched to the question.
    ///
    /// The OTHER half of the rule — an obsolete run may not mount at all — is the host's, and cannot be enforced here:
    /// generations are numbered per host component instance, so a legitimate new instance starts again at 1 and Rust
    /// must not refuse it. What Rust can and does enforce is that a RESPONSE only lands on the mount that asked
    /// (`a_response_for_another_screen_or_another_mount_is_discarded`).
    #[test]
    fn a_mount_stamps_its_generation_on_the_requests_it_makes() {
        let mut program = Program::new();
        let effects = program.dispatch(Msg::Mount {
            screen: target("site-home"),
            generation: 3,
        });
        assert_eq!(program.model().generation, 3);
        assert_eq!(
            effects,
            vec![Effect::FetchPage {
                screen: "site-home",
                scope: None,
                generation: 3,
            }],
            "the request carries the mount that asked for it"
        );
    }

    #[test]
    fn the_host_decides_which_screen_opens() {
        let mut program = Program::new();
        let effects = program.open(target("dashboard"));
        assert_eq!(program.model().screen, target("dashboard"));
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "dashboard",
                scope: None,
                generation: 0
            }]
        );
    }

    /// The table is the source of truth for the whole port, so it has to be internally consistent: unique keys, one
    /// screen per live route, records that point at a real list screen, and a surface for everything.
    #[test]
    fn the_screen_table_is_consistent() {
        let mut keys: Vec<&str> = SCREENS.iter().map(|s| s.key).collect();
        keys.sort_unstable();
        let counted = keys.len();
        keys.dedup();
        assert_eq!(
            counted,
            keys.len(),
            "two screens share a key, so addressing one is ambiguous"
        );

        let mut paths: Vec<&str> = SCREENS.iter().map(|s| s.path).collect();
        paths.sort_unstable();
        let counted_paths = paths.len();
        paths.dedup();
        // This is not hypothetical: the first draft of the table listed /portal/settings twice, under two surfaces.
        assert_eq!(
            counted_paths,
            paths.len(),
            "two screens claim the same live route"
        );

        for screen in SCREENS {
            // Empty is allowed and pinned by name in `the_table_matches_the_route_tree`; anything else must be a route.
            assert!(
                screen.path.is_empty() || screen.path.starts_with('/'),
                "{} has a path that is not a route: {:?}",
                screen.key,
                screen.path
            );
            assert!(!screen.title.is_empty(), "{} has no title", screen.key);
            match screen.detail_of {
                Some(list) => {
                    assert_eq!(
                        screen.nav,
                        Nav::Record,
                        "{} is opened from a row, so it is not a nav entry",
                        screen.key
                    );
                    assert!(
                        crate::model::screen(list).is_some(),
                        "{} is opened from '{list}', which is not a screen in the table",
                        screen.key
                    );
                }
                None => assert_ne!(
                    screen.nav,
                    Nav::Record,
                    "{} is a record that nothing can open",
                    screen.key
                ),
            }
        }
    }

    /// A record screen is reached by opening a row, never from the nav: "one client, but which one?" is not something
    /// a menu can offer.
    #[test]
    fn a_record_screen_is_never_in_the_nav() {
        for screen in SCREENS {
            let records: Vec<Screen> = SCREENS
                .iter()
                .copied()
                .filter(|candidate| candidate.detail_of == Some(screen.key))
                .collect();
            // One click cannot mean two things: when a list has more than one record, neither is offered by a row
            // (`workflows` has both an instance record and a runtime inspector).
            if records.len() == 1 {
                assert!(
                    !records[0].is_listed(),
                    "{} is opened from a row, so it must not be a nav entry",
                    records[0].key
                );
                assert_eq!(
                    records[0].surface, screen.surface,
                    "a record stays in its own surface"
                );
            }
        }
    }

    /// THE TABLE IS CHECKED AGAINST THE ROUTE TREE, not against a registry.
    ///
    /// `lib/navigation/registry.ts` says of itself "Only EXISTING routes are listed" — a statement about navigation,
    /// and I read it as a statement about scope. That mistake cost nine real pages (login, /auth/error, the dev map
    /// tests, the token review page, the portal root, the auth proof page) which were never in the table at all.
    /// The filesystem does not have opinions, so this walks it.
    ///
    /// The two exceptions are this port's own preview hosts, listed here with the reason.
    #[test]
    fn the_table_matches_the_route_tree() {
        const PREVIEW_HOSTS: [&str; 2] = ["/portal/rust-preview", "/rust-preview"];

        let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
        let mut routes = Vec::new();
        collect_routes(&app, String::new(), &mut routes);

        assert!(
            !routes.is_empty(),
            "found no routes under {} — is the path wrong?",
            app.display()
        );

        // Collect everything, then assert once: a check that stops at the first missing route makes fixing them a
        // guessing game of how many runs it will take.
        let mut missing: Vec<&String> = routes
            .iter()
            .filter(|route| !PREVIEW_HOSTS.contains(&route.as_str()))
            .filter(|route| !SCREENS.iter().any(|screen| screen.path == route.as_str()))
            .collect();
        missing.sort();
        assert!(
            missing.is_empty(),
            "{} route(s) with no screen in the table — add rows; do not narrow the scope:\n  {}",
            missing.len(),
            missing
                .iter()
                .map(|route| route.as_str())
                .collect::<Vec<_>>()
                .join("\n  ")
        );

        // A screen with no live route (path is empty) is skipping the route comparison, and it is REPORTED rather than
        // quietly exempt: the point of this test is that nothing is hidden.
        let orphans: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| !screen.path.is_empty())
            .map(|screen| screen.path)
            .filter(|path| !routes.iter().any(|route| route == path))
            .collect();
        assert!(
            orphans.is_empty(),
            "screens claiming routes that do not exist: {orphans:?}"
        );

        let no_route: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| screen.path.is_empty())
            .map(|screen| screen.key)
            .collect();
        // Not a failure — a screen over data the app exposes through no route of its own is legitimate. But it must be
        // deliberate, so it is listed here and the number is asserted rather than allowed to grow unnoticed.
        //
        // EMPTY, and that is the news: `site-properties` was the last one. It sat here with an empty path while the
        // public read model was reachable only through `/properties/[slug]`, and the most important page on the site
        // had no address. It has one now (`app/properties/page.tsx`), so nothing in the table is route-less.
        assert_eq!(
            no_route,
            Vec::<&str>::new(),
            "a screen with no live route must be a deliberate choice, and this is the list of them"
        );
    }

    /// Walk `app/` for `page.tsx` files, building the route each one serves. `/` is the root page.
    fn collect_routes(dir: &std::path::Path, prefix: String, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                collect_routes(&path, format!("{prefix}/{name}"), out);
            } else if path.file_name().is_some_and(|name| name == "page.tsx") {
                out.push(if prefix.is_empty() {
                    "/".to_string()
                } else {
                    prefix.clone()
                });
            }
        }
    }

    /// Every surface the registry defines is reachable from the switcher, and no surface is a dead end.
    #[test]
    fn every_surface_has_a_home() {
        for surface in Surface::ALL.iter().copied() {
            let home = SCREENS
                .iter()
                .copied()
                .find(|candidate| candidate.surface == surface && candidate.is_listed());
            assert!(
                home.is_some(),
                "{} has no listed screen to open",
                surface.label()
            );
        }
    }
}
