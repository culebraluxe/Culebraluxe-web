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

pub mod calendar;
pub mod flight_recorder;
pub mod format;
pub mod model;
pub mod navigation;
pub mod ops;
pub mod projects;
pub mod search;
pub mod seller_strategy;
pub mod timeline;

#[cfg(feature = "wasm")]
pub mod shell;

/// The application framework and master shell (docs/agent/UI-SCREEN-ARCHITECTURE.md).
#[cfg(all(feature = "wasm", feature = "yew", feature = "yew-router"))]
pub mod app;

pub mod icons;
pub use model::{
    home, listed, record_for, screen, screen_for_path, Block, BlockItem, Controls, Listing, Nav,
    PageContent, Row, Screen, Surface, PAGE_SIZE, SCREENS,
};
