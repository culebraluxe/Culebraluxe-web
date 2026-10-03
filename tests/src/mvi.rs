//! MVI helpers: a deterministic driver for Model → Message → update reducers, and a harness for the real
//! `ui::app::Screen` boundary.
//!
//! The pattern is MVI — state (`Model`), an intent (`Msg`), and a pure `update` that turns one into the other. Because
//! `init` and `update` are pure, a screen can be driven without a browser: [`screen::ScreenHarness`] runs the same
//! `init`/`update` the browser runs and records what the screen asked the shell to do (`Cmd`, side effects as data).
//!
//! [`Reducer`]/[`Driver`] are the transport-independent half: any pure state machine, including a domain reducer
//! that never touches a `Screen`, is driven the same way and gets the same loop protection (`drain` has a step
//! ceiling, so a reducer that emits a message for every message cannot spin forever).

use std::collections::VecDeque;

/// A pure MVI reducer: `init` produces state and any first messages; `update` mutates state and returns follow-ups.
pub trait Reducer {
    /// The whole state of the machine.
    type State: Clone + PartialEq;
    /// Everything that can happen to it.
    type Msg;

    /// The first state, and any messages the first state should receive.
    fn init() -> (Self::State, Vec<Self::Msg>);

    /// Apply one message; return the messages it produced.
    fn update(state: &mut Self::State, msg: Self::Msg) -> Vec<Self::Msg>;
}

/// Drives a [`Reducer`] deterministically.
pub struct Driver<R: Reducer> {
    state: R::State,
    pending: VecDeque<R::Msg>,
    updates: usize,
}

impl<R: Reducer> Driver<R> {
    /// Build the machine and queue any messages `init` produced.
    pub fn start() -> Self {
        let (state, followups) = R::init();
        Self {
            state,
            pending: followups.into(),
            updates: 0,
        }
    }

    /// The current state.
    pub fn state(&self) -> &R::State {
        &self.state
    }

    /// How many messages are queued but not yet applied.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// How many updates have been applied.
    pub fn updates(&self) -> usize {
        self.updates
    }

    /// Queue a message.
    pub fn enqueue(&mut self, msg: R::Msg) {
        self.pending.push_back(msg);
    }

    /// Apply one message immediately, queueing its follow-ups. Returns how many follow-ups it produced.
    pub fn dispatch(&mut self, msg: R::Msg) -> usize {
        let followups = R::update(&mut self.state, msg);
        self.updates += 1;
        let count = followups.len();
        self.pending.extend(followups);
        count
    }

    /// Apply queued messages until none remain or `max_steps` updates have run. Returns the steps applied.
    ///
    /// The ceiling is the loop guard: a reducer that always emits a follow-up would otherwise hang the test, and a
    /// hang is a worse failure than an assertion.
    pub fn drain(&mut self, max_steps: usize) -> usize {
        let mut steps = 0;
        while steps < max_steps {
            let Some(msg) = self.pending.pop_front() else {
                break;
            };
            self.dispatch(msg);
            steps += 1;
        }
        steps
    }
}

/// The harness for the production MVI boundary: `ui::app::Screen`.
pub mod screen {
    use ui::app::cmd::Cmd;
    use ui::app::screen::{Screen, ScreenCtx};

    /// Drives one real `Screen` through `init`/`update`, with its context.
    ///
    /// It never calls `view` and never touches the DOM: `view` is a render, and a contract test asserts on state and
    /// requested effects, not on HTML.
    pub struct ScreenHarness<S: Screen> {
        model: S::Model,
        ctx: ScreenCtx,
        updates: usize,
    }

    impl<S: Screen> ScreenHarness<S> {
        /// Open a screen: run its `init` with `ctx` and return the harness plus the screen's first commands.
        pub fn open(ctx: ScreenCtx) -> (Self, Cmd<S::Msg>) {
            let (model, command) = S::init(&ctx);
            (
                Self {
                    model,
                    ctx,
                    updates: 0,
                },
                command,
            )
        }

        /// The screen's current model.
        pub fn model(&self) -> &S::Model {
            &self.model
        }

        /// The context the screen was opened with.
        pub fn ctx(&self) -> &ScreenCtx {
            &self.ctx
        }

        /// How many messages have been applied.
        pub fn updates(&self) -> usize {
            self.updates
        }

        /// Deliver one message to the screen and return the commands it asked for.
        pub fn update(&mut self, msg: S::Msg) -> Cmd<S::Msg> {
            self.updates += 1;
            S::update(&mut self.model, msg, &self.ctx)
        }

        /// Deliver several messages in order, collecting each message's commands.
        pub fn update_all(
            &mut self,
            messages: impl IntoIterator<Item = S::Msg>,
        ) -> Vec<Cmd<S::Msg>> {
            messages.into_iter().map(|msg| self.update(msg)).collect()
        }

        /// Tell the screen the URL query changed while it stays open.
        pub fn url_changed(&mut self) -> Cmd<S::Msg> {
            S::url_changed(&mut self.model, &self.ctx)
        }
    }

    /// A command's kind, as a value a test can assert on without naming `Cmd`'s payloads.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CommandKind {
        None,
        Batch,
        Request,
        Navigate,
        Load,
        ReplacePath,
        SharePdf,
        Listen,
        StorageRead,
        StorageWrite,
        After,
        Upload,
        VideoUpload,
        PostForm,
    }

    /// Flatten a command tree into its kinds, depth-first. A `Batch` contributes itself and then its children.
    pub fn classify<Msg>(command: &Cmd<Msg>) -> Vec<CommandKind> {
        fn walk<Msg>(command: &Cmd<Msg>, out: &mut Vec<CommandKind>) {
            match command {
                Cmd::None => out.push(CommandKind::None),
                Cmd::Batch(children) => {
                    out.push(CommandKind::Batch);
                    for child in children {
                        walk(child, out);
                    }
                }
                Cmd::Request(_) => out.push(CommandKind::Request),
                Cmd::Navigate(_) => out.push(CommandKind::Navigate),
                Cmd::Load(_) => out.push(CommandKind::Load),
                Cmd::ReplacePath(_) => out.push(CommandKind::ReplacePath),
                Cmd::SharePdf { .. } => out.push(CommandKind::SharePdf),
                Cmd::Listen { .. } => out.push(CommandKind::Listen),
                Cmd::StorageRead { .. } => out.push(CommandKind::StorageRead),
                Cmd::StorageWrite { .. } => out.push(CommandKind::StorageWrite),
                Cmd::After { .. } => out.push(CommandKind::After),
                Cmd::Upload(_) => out.push(CommandKind::Upload),
                Cmd::VideoUpload(_) => out.push(CommandKind::VideoUpload),
                Cmd::PostForm { .. } => out.push(CommandKind::PostForm),
            }
        }
        let mut kinds = Vec::new();
        walk(command, &mut kinds);
        kinds
    }

    /// The first non-`None`, non-`Batch` command kind, if any.
    pub fn effect_kind<Msg>(command: &Cmd<Msg>) -> Option<CommandKind> {
        classify(command)
            .into_iter()
            .find(|kind| !matches!(kind, CommandKind::None | CommandKind::Batch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A counter that increments, plus a message that always queues itself.
    struct Counter;

    #[derive(Debug, Clone, PartialEq)]
    struct State {
        value: i32,
    }

    enum Msg {
        Increment,
        Spin,
    }

    impl Reducer for Counter {
        type State = State;
        type Msg = Msg;

        fn init() -> (State, Vec<Msg>) {
            (State { value: 0 }, vec![Msg::Increment])
        }

        fn update(state: &mut State, msg: Msg) -> Vec<Msg> {
            match msg {
                Msg::Increment => {
                    state.value += 1;
                    Vec::new()
                }
                // A self-perpetuating message: applying it always queues another.
                Msg::Spin => vec![Msg::Spin],
            }
        }
    }

    #[test]
    fn a_reducer_is_driven_deterministically() {
        let mut driver = Driver::<Counter>::start();
        assert_eq!(driver.state().value, 0);
        assert_eq!(driver.pending(), 1, "init queued the first increment");
        assert_eq!(driver.drain(10), 1);
        assert_eq!(driver.state().value, 1);

        assert_eq!(driver.dispatch(Msg::Increment), 0);
        assert_eq!(driver.state().value, 2);
        assert_eq!(driver.updates(), 2);
    }

    #[test]
    fn the_drain_ceiling_guards_a_self_perpetuating_reducer() {
        let mut driver = Driver::<Counter>::start();
        driver.drain(10);
        // `Spin` always emits another `Spin`, so an unbounded drain would never end.
        driver.enqueue(Msg::Spin);
        let steps = driver.drain(5);
        assert_eq!(steps, 5, "the ceiling stopped the loop");
        assert!(driver.pending() > 0);
    }

    #[test]
    fn a_real_screen_is_driven_without_a_browser() {
        use ui::app::cmd::ApiError;
        use ui::app::screen::ScreenCtx;
        use ui::app::screens::db_test::{DbTest, Msg as DbTestMsg};

        let (mut harness, command) = screen::ScreenHarness::<DbTest>::open(ScreenCtx::default());
        assert_eq!(
            screen::classify(&command),
            vec![screen::CommandKind::Request],
            "opening the db-test screen asks the shell for its read"
        );

        let failed = ApiError::network("harness proves the reducer path");
        let follow_up = harness.update(DbTestMsg::Loaded(Err(failed.clone())));
        assert_eq!(
            screen::classify(&follow_up),
            vec![screen::CommandKind::None]
        );
        assert_eq!(
            harness.model().read,
            ui::app::cmd::Remote::Failed(failed),
            "the screen stores the failure in its model"
        );
        assert_eq!(harness.updates(), 1);
    }
}
