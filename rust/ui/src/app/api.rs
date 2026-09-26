//! THE API CATALOGUE — every URL the app calls, and nowhere else.
//!
//! Screens name an endpoint (`Cmd::request(PortalScreenPage::of("db-test"), ...)`); they never write a path. When the HTTP
//! layer moves from the Next relays to Axum, the paths change here and no screen changes.

use serde::Deserialize;

use crate::app::cmd::{Endpoint, Method};

/// A portal screen's page payload (`/api/portal/rust-ui/page`). The answer is the portal page itself (`{ support, ... }`),
/// not wrapped in the site's `PageContent`.
pub struct PortalScreenPage {
    pub screen: &'static str,
}

impl PortalScreenPage {
    pub fn of(screen: &'static str) -> Self {
        Self { screen }
    }
}

impl Endpoint for PortalScreenPage {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        format!("/api/portal/rust-ui/page?screen={}&", self.screen)
    }
}

/// Auth.js's form token, which every form posting to Auth.js must carry.
pub struct AuthCsrf;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsrfToken {
    pub csrf_token: String,
}

impl Endpoint for AuthCsrf {
    const METHOD: Method = Method::Get;
    type Response = CsrfToken;
    fn path(&self) -> String {
        "/api/auth/csrf".into()
    }
}

/// Who is signed in on the public site (provisions the external guest on a first sign-in).
pub struct GuestWhoAmI;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuestSession {
    pub signed_in: bool,
    pub display_name: String,
    pub email: Option<String>,
}

impl Endpoint for GuestWhoAmI {
    const METHOD: Method = Method::Get;
    type Response = GuestSession;
    fn path(&self) -> String {
        "/api/rust-ui/guest".into()
    }
}

/// Email a guest a sign-in code.
pub struct GuestRequestCode {
    pub email: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct CodeSent {
    pub sent: bool,
}

impl Endpoint for GuestRequestCode {
    const METHOD: Method = Method::Post;
    type Response = CodeSent;
    fn path(&self) -> String {
        "/api/rust-ui/guest".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "email": self.email }))
    }
}

/// Percent-encode one query value.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
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
