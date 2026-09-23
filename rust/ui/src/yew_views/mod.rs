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
pub mod portal_accounting_dashboard;
pub mod portal_accounting_expenses;
pub mod portal_accounting_pnl;
pub mod portal_accounting_receipt_scanner;
pub mod portal_accounting_receivables;
pub mod portal_accounting_shell;
pub mod home;
pub mod portal_activity;
pub mod portal_cabinet;
pub mod portal_clients;
pub mod portal_cockpit;
pub mod portal_deals;
pub mod portal_forms;
pub mod portal_flight_recorder;
pub mod portal_flight_recorder_list;
pub mod portal_projects;
pub mod portal_records;
pub mod portal_listing_media;
pub mod portal_ops;
pub mod portal_seller_strategy;
pub mod portal_storyboard;
pub mod portal_shell;
pub mod portal_support_db_test;
pub mod portal_support_security;
pub mod portal_support_system_health;
pub mod portal_support_whatsapp_meta;
pub mod portal_ui_lab;
pub mod portal_tech;
pub mod sellers;
pub mod services;

pub mod portal_workflow_record;
pub mod portal_workflows;
