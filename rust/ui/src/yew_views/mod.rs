//! The Yew views, one module per screen.
//!
//! Each is a component that takes the model and a dispatch callback and returns `Html` — the same `Model -> view`
//! shape the string renderers have, with a virtual DOM diffing the result instead of a string being written over the
//! page. The chrome lives here too, because the header and footer belong to every one of them.

pub mod about;
pub mod buyers;
pub mod chrome;
pub mod contact;
pub mod faq;
pub mod favorites;
pub mod guide;
pub mod home;
pub mod portal_flight_recorder_list;
pub mod portal_forms;
pub mod portal_shell;
pub mod property_detail;
pub mod sellers;
pub mod services;
