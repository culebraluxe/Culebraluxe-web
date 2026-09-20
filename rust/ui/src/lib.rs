//! The Rust UI, built on MVI: Model → View → Intent, one direction round.
//!
//! WHY MVI AND NOT "A RUST APP WITH SOME STATE": the pattern makes the screen a pure function of data. `Model` is
//! the whole screen state, `Msg` is everything that can happen to it, `update` is the only thing that changes it
//! and it is pure, and `view` renders the model without deciding anything. That buys two things this repository
//! already insists on everywhere else — the interaction model is testable without a browser, and a UI bug is a
//! reducer bug with a named message instead of a mystery in a lifecycle hook.
//!
//! THE RULES THIS CRATE KEEPS, in the same spirit as the Rust port's other boundaries:
//!
//! 1. **One owner of application state.** The Rust model owns what is selected, what is loading, and what the
//!    server said. A third-party widget — the tree, the Gantt, the calendar — owns only its own rendering and local
//!    mechanics, receives a snapshot, and reports intents back. It never keeps a competing project model and never
//!    saves anything itself.
//! 2. **Strict DOM ownership.** Rust owns a widget's container element; the widget owns everything inside it.
//!    Neither framework edits the other's DOM. (Enforced by the shell when it lands, not by this crate.)
//! 3. **A small typed bridge.** Rust and any widget exchange plain data and named events keyed by stable ids. No
//!    widget handles, no component objects, no callbacks holding Rust state.
//! 4. **The server stays authoritative.** This crate never holds a credential and never decides permissions. It
//!    calls the application's own routes, which own the session and the server-side service credentials; a rejected
//!    change is expressed as a message (`EffectFailed`) that restores the model, which is how a widget snaps back.
//!
//! Nothing here is wired into the site yet. The first proof is one simple screen with selection working in both
//! directions; the Project Management screen, with its third-party widgets, is deliberately last.

pub mod model;
pub mod update;
pub mod view;

pub use model::{Effect, Model, Msg, PropertySummary, Screen};
pub use update::update;
pub use view::render;

/// The whole program, so a host never has to remember the loop's shape.
///
/// A host — the WASM shell later, a test right now — holds a `Model` and feeds it messages. `dispatch` returns the
/// effects the caller must run, and none of them are permissions: the model cannot decide what it is allowed to do,
/// only what it would like to ask for.
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

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Apply one intent, returning the effects the host has to perform. The model is already updated when this
    /// returns, so the view can be re-rendered immediately — the request is a consequence of the state change,
    /// never a precondition for it.
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
    fn a_new_program_starts_on_the_first_screen_with_no_data_and_no_selection() {
        let program = Program::new();
        assert_eq!(program.model().screen, Screen::Portfolio);
        assert!(program.model().properties.is_empty());
        assert_eq!(program.model().selected_property_id, None);
        assert!(!program.model().loading);
    }

    #[test]
    fn dispatch_changes_the_model_and_returns_the_effect_to_run() {
        let mut program = Program::new();
        let effects = program.dispatch(Msg::ScreenOpened);
        assert!(
            program.model().loading,
            "the model must reflect the intent immediately"
        );
        assert_eq!(effects, vec![Effect::LoadPortfolio]);
    }

    #[test]
    fn the_view_renders_the_model_it_was_given() {
        let mut program = Program::new();
        let before = program.html();
        program.dispatch(Msg::PortfolioLoaded(vec![PropertySummary {
            id: "p1".into(),
            label: "Bo. Delicias 17a".into(),
            owner_known: true,
            acreage: Some(1.25),
        }]));
        let after = program.html();
        assert_ne!(
            before, after,
            "state that does not change the view is state nothing needed"
        );
        assert!(after.contains("Bo. Delicias 17a"));
    }
}
