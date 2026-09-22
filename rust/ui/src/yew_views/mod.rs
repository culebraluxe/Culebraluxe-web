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
pub mod guide;
pub mod home;
pub mod portal_activity;
pub mod portal_clients;
pub mod portal_cockpit;
pub mod portal_forms;
pub mod portal_projects;
pub mod portal_shell;
pub mod sellers;
pub mod services;

pub mod portal_workflow_record;
pub mod portal_workflows;
