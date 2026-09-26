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

/// What the signed-in portal user may do, as the Rust security service answers. Read once per portal visit by the shell.
pub struct Entitlements;

impl Endpoint for Entitlements {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalEntitlements;
    fn path(&self) -> String {
        "/api/portal/rust-ui/entitlements".into()
    }
}

/// ROOT grants or revokes one entitlement for one internal role. Answers the role table as it now stands.
pub struct SetRoleEntitlement {
    pub role_code: String,
    pub action: String,
    pub granted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RolesAnswer {
    pub roles: Vec<crate::model::PortalRoleEntitlements>,
}

impl Endpoint for SetRoleEntitlement {
    const METHOD: Method = Method::Put;
    type Response = RolesAnswer;
    fn path(&self) -> String {
        "/api/portal/rust-ui/role-entitlements".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({ "roleCode": self.role_code, "action": self.action, "granted": self.granted }),
        )
    }
}

/// ROOT replaces one internal user's primary role. Answers the user table as it now stands.
pub struct SetUserPrimaryRole {
    pub app_user_id: String,
    pub role_code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct UsersAnswer {
    pub users: Vec<crate::model::PortalSecurityUser>,
}

impl Endpoint for SetUserPrimaryRole {
    const METHOD: Method = Method::Put;
    type Response = UsersAnswer;
    fn path(&self) -> String {
        "/api/portal/rust-ui/security-users".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "appUserId": self.app_user_id, "roleCode": self.role_code }))
    }
}

/// A generic rows read: the portal's (`/api/portal/rust-ui/rows`) or the public site's (`/api/rust-ui/public-rows`).
pub struct RowsRead {
    pub public: bool,
    pub screen: &'static str,
}

impl RowsRead {
    pub fn portal(screen: &'static str) -> Self {
        Self {
            public: false,
            screen,
        }
    }

    pub fn public(screen: &'static str) -> Self {
        Self {
            public: true,
            screen,
        }
    }
}

impl Endpoint for RowsRead {
    const METHOD: Method = Method::Get;
    type Response = Vec<crate::model::Row>;
    fn path(&self) -> String {
        let base = if self.public {
            "/api/rust-ui/public-rows"
        } else {
            "/api/portal/rust-ui/rows"
        };
        format!("{base}?screen={}", self.screen)
    }
}

/// The P&L for a period (`from`/`to` as `YYYY-MM-DD`). Empty ends are the server's to fill: a first open asks for nothing
/// and gets the current month, which the answer then names. Answers the portal page (`{ accounting: ... }`).
pub struct AccountingPnl {
    pub from: String,
    pub to: String,
}

impl Endpoint for AccountingPnl {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/page?screen=accounting-pnl&from={}&to={}",
            encode(&self.from),
            encode(&self.to)
        )
    }
}

/// One Accounting write (`createExpense`, `createReceivable`, `markReceivablePaid`). The body names the screen it came
/// from, and the answer is that screen's refreshed page — the new row is already in it.
pub struct AccountingCommand {
    pub body: serde_json::Value,
}

impl Endpoint for AccountingCommand {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/accounting".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

/// The Cockpit's read: KPIs, tasks, the featured deal, the pipeline and recent interactions. Answers `{ cockpit: ... }`.
pub struct CockpitRead;

impl Endpoint for CockpitRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/cockpit".into()
    }
}

/// Mark one Cockpit task done. Answers the refreshed Cockpit.
pub struct CockpitCompleteTask {
    pub task_id: String,
}

impl Endpoint for CockpitCompleteTask {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/cockpit".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "action": "completeTask", "taskId": self.task_id }))
    }
}

/// The Cabinet: every issued document. Answers `{ cabinet: ... }`.
pub struct CabinetRead;

impl Endpoint for CabinetRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/cabinet".into()
    }
}
