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
//! SCOPE: the portal menu. `Screen::ALL` is the port's to-do list, `Screen::portal_path()` names the live route each
//! entry replaces, and Project Management is deliberately a placeholder.

pub mod model;
pub mod update;
pub mod view;

#[cfg(feature = "wasm")]
pub mod shell;

pub use model::{Area, Effect, Model, Msg, Row, Screen};
pub use update::update;
pub use view::render;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_program_opens_on_the_first_menu_screen_with_nothing_loaded() {
        let program = Program::new();
        assert_eq!(program.model().screen, Screen::ALL[0]);
        assert!(program.model().rows.is_empty());
        assert!(!program.model().loading);
    }

    #[test]
    fn opening_a_screen_asks_the_host_for_its_rows() {
        let mut program = Program::new();
        let effects = program.dispatch(Msg::ScreenOpened(Screen::Clients));
        assert!(
            program.model().loading,
            "the model must reflect the intent immediately"
        );
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "clients",
                scope: None
            }]
        );
    }

    #[test]
    fn the_view_renders_the_model_it_was_given() {
        let mut program = Program::new();
        let before = program.html();
        program.dispatch(Msg::RowsLoaded(vec![Row {
            id: "c1".into(),
            cells: vec!["Ada Lovelace".into(), "Buyer".into()],
            badge: Some("new".into()),
        }]));
        let after = program.html();
        assert_ne!(
            before, after,
            "state that does not change the view is state nothing needed"
        );
        assert!(after.contains("Ada Lovelace"));
    }

    #[test]
    fn the_host_decides_which_screen_opens() {
        let mut program = Program::new();
        let effects = program.open(Screen::Dashboard);
        assert_eq!(program.model().screen, Screen::Dashboard);
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "dashboard",
                scope: None
            }]
        );
    }

    #[test]
    fn the_two_areas_do_not_overlap() {
        let site: Vec<Screen> = Screen::ALL
            .iter()
            .copied()
            .filter(|s| s.area() == Area::Site)
            .collect();
        let portal: Vec<Screen> = Screen::ALL
            .iter()
            .copied()
            .filter(|s| s.area() == Area::Portal)
            .collect();
        assert_eq!(
            site.len() + portal.len(),
            Screen::ALL.len(),
            "every screen belongs to an area"
        );
        assert!(site.iter().all(|s| !portal.contains(s)));
        assert!(site.iter().all(|s| s.live_path().starts_with('/')));
    }

    /// A record screen is reached by opening a row, never from the nav: "one client, but which one?" is not something
    /// a menu can offer. Checked for every screen that declares a detail view, so adding one to `ALL` by hand fails
    /// here rather than shipping a nav entry that needs an argument nobody can supply.
    #[test]
    fn a_record_screen_is_never_in_the_nav() {
        for screen in Screen::ALL {
            if let Some(detail) = screen.detail() {
                assert!(
                    !Screen::ALL.contains(&detail),
                    "{} is opened from a row, so it must not be a nav entry",
                    detail.key()
                );
                assert_eq!(
                    detail.area(),
                    screen.area(),
                    "a detail view stays in its own area"
                );
            }
        }
    }
}
