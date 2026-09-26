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
pub mod portal_flight_recorder;
pub mod portal_flight_recorder_list;
pub mod portal_forms;
pub mod portal_listing_media;
pub mod portal_ops;
pub mod portal_projects;
pub mod portal_records;
pub mod portal_shell;
pub mod portal_storyboard;
pub mod portal_tech;
pub mod portal_ui_lab;
pub mod property_detail;
pub mod sellers;
pub mod services;
