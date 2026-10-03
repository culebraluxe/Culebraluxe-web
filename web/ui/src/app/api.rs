//! THE API CATALOGUE — every URL the app calls, and nowhere else.
//!
//! Screens name an endpoint (`Cmd::request(PortalScreenPage::of("db-test"), ...)`); they never write a path — including
//! for files (`FileEndpoint`: chunked uploads and multipart forms). Every path here is answered by the Rust server.
//!
//! Answers are decoded from JSON TEXT with `from_str`, not from a `serde_json::Value`: decoding from a `Value` compiled a
//! second full deserializer per payload type and doubled the wasm (see `cmd::Request`). Do not "simplify" it back.

use crate::app::cmd::FileEndpoint;

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::app::cmd::{Endpoint, Method};
mod addresses;
mod forms;
mod guest;
mod portal;
mod projects_ops;
#[allow(unused_imports)]
pub use addresses::*;
#[allow(unused_imports)]
pub use forms::*;
#[allow(unused_imports)]
pub use guest::*;
#[allow(unused_imports)]
pub use portal::*;
#[allow(unused_imports)]
pub use projects_ops::*;

/// A portal screen's page payload (`/api/portal/rust-ui/page`). The answer is the portal page itself (`{ support, ... }`),
/// not wrapped in the site's `PageContent`.
pub struct PortalScreenPage {
    pub screen: &'static str,
    /// The record the read is about (System Health: the workflow instance whose row is open).
    pub scope: Option<String>,
}

impl PortalScreenPage {
    pub fn of(screen: &'static str) -> Self {
        Self {
            screen,
            scope: None,
        }
    }

    pub fn scoped(screen: &'static str, scope: impl Into<String>) -> Self {
        Self {
            screen,
            scope: Some(scope.into()),
        }
    }
}

impl Endpoint for PortalScreenPage {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        match &self.scope {
            Some(scope) => format!(
                "/api/portal/rust-ui/page?screen={}&scope={}",
                self.screen,
                encode(scope)
            ),
            None => format!("/api/portal/rust-ui/page?screen={}", self.screen),
        }
    }
}

/// The client directory with one person hydrated, or (with `record`) just that person. Answers `{ clients: ... }`.
pub struct ClientsRead {
    /// A client record route's id: read only that person.
    pub record: Option<String>,
    pub selected: Option<String>,
    pub search: String,
    /// 1-based, as the list counts; the relay counts from 0.
    pub page: i64,
}

impl Endpoint for ClientsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        if let Some(id) = &self.record {
            return format!(
                "/api/portal/rust-ui/clients?screen=client-record&scope={}",
                encode(id)
            );
        }
        let mut path = format!(
            "/api/portal/rust-ui/clients?screen=clients&page={}&search={}",
            (self.page - 1).max(0),
            encode(&self.search)
        );
        if let Some(selected) = &self.selected {
            path.push_str(&format!("&selected={}", encode(selected)));
        }
        path
    }
}

// ---------------------------------------------------------------- Forms
