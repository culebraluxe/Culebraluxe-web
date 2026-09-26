//! The Yew views, one module per screen.
//!
//! Each is a component that takes the model and a dispatch callback and returns `Html` — the same `Model -> view`
//! shape the string renderers have, with a virtual DOM diffing the result instead of a string being written over the
//! page. The chrome lives here too, because the header and footer belong to every one of them.

pub mod chrome;
pub mod portal_flight_recorder_list;
pub mod portal_forms;
pub mod portal_shell;
