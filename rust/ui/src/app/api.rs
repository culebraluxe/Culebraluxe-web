//! THE API CATALOGUE — every URL the app calls, and nowhere else.
//!
//! Screens name an endpoint (`Cmd::request(PortalPage::of("db-test"), ...)`); they never write a path. When the HTTP
//! layer moves from the Next relays to Axum, the paths change here and no screen changes.

use serde::Deserialize;

use crate::app::cmd::{Endpoint, Method};

/// A portal screen's page payload (`/api/portal/rust-ui/page`), the typed read the old loop used for SUPPORT screens.
pub struct PortalPage {
    pub screen: &'static str,
}

impl PortalPage {
    pub fn of(screen: &'static str) -> Self {
        Self { screen }
    }
}

impl Endpoint for PortalPage {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PageContent;
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
