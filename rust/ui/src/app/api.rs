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

/// The Contracts portfolio (`screen=deals`), or one deal's workspace (`screen=deal-record&scope=<dealId>`).
/// Answers `{ deals: ... }`.
pub struct DealsRead {
    pub deal_id: Option<String>,
}

impl Endpoint for DealsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        match &self.deal_id {
            Some(id) => format!(
                "/api/portal/rust-ui/deals?screen=deal-record&scope={}",
                encode(id)
            ),
            None => "/api/portal/rust-ui/deals?screen=deals".into(),
        }
    }
}

/// People who can be put on a deal, by name. Answers `{ people: [...] }`.
pub struct DealPeopleSearch {
    pub query: String,
}

impl Endpoint for DealPeopleSearch {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalDealPeopleSearch;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/deals?peopleSearch={}",
            encode(&self.query)
        )
    }
}

/// What a create or a deal command answers: the id of the record it made or changed.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RecordId {
    pub id: String,
}

/// Open a deal for a property and an existing client.
pub struct DealCreate {
    pub property_id: String,
    pub client_person_id: String,
    pub owner_user_id: Option<String>,
    pub notes: Option<String>,
}

impl Endpoint for DealCreate {
    const METHOD: Method = Method::Post;
    type Response = RecordId;
    fn path(&self) -> String {
        "/api/portal/rust-ui/deals".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "propertyId": self.property_id,
            "clientPersonId": self.client_person_id,
            "ownerUserId": self.owner_user_id,
            "notes": self.notes,
        }))
    }
}

/// One command on a deal's workspace: tasks, offers, showings, participants.
pub struct DealWorkspaceCommand {
    pub deal_id: String,
    pub command: crate::model::PortalDealCommand,
}

impl Endpoint for DealWorkspaceCommand {
    const METHOD: Method = Method::Post;
    type Response = RecordId;
    fn path(&self) -> String {
        format!("/api/portal/rust-ui/deals?scope={}", encode(&self.deal_id))
    }
    fn body(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.command).ok()
    }
}

/// One process instance's recorded transaction, as the Flight Recorder console reads it. The answer is passed to the
/// console untouched, so it stays JSON here.
pub struct FlightRecorderRead {
    pub instance_id: String,
}

impl Endpoint for FlightRecorderRead {
    const METHOD: Method = Method::Get;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        format!("/api/portal/flight-recorder/{}", encode(&self.instance_id))
    }
}

/// The Forge Cockpit, with one story's detail when `selected` is set. Answers `{ tech: ... }`.
pub struct TechRead {
    pub selected: Option<String>,
}

impl Endpoint for TechRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        match self.selected.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => format!("/api/portal/rust-ui/tech?selected={}", encode(id)),
            None => "/api/portal/rust-ui/tech".into(),
        }
    }
}

/// One Cockpit command (`clearWorkbench`, `goodToGo`, `scopedRun`, `moveWorkbench`, `launchFlight`, `scheduleFlight`,
/// `cancelFlight`). Answers `{ ok, message }`.
pub struct TechCommand {
    pub body: serde_json::Value,
}

impl Endpoint for TechCommand {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/portal/rust-ui/tech".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

/// Project Management: every project with its work items, documents, media, activity and calendar. Answers
/// `{ projects: ... }`.
pub struct ProjectsRead;

impl Endpoint for ProjectsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects".into()
    }
}

/// One Projects write (`projectStatus`, `wbsSave`). Answers the refreshed projects page.
pub struct ProjectsCommand {
    pub body: serde_json::Value,
}

impl Endpoint for ProjectsCommand {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

/// The Data Workbench: one page of an entity's records (`property`, `person`, `project`), with one opened when
/// `selected` is set. Answers `{ ops: ... }`.
pub struct OpsRead {
    pub entity: String,
    pub selected: Option<String>,
    pub search: String,
    /// 0-based, as the relay counts.
    pub page: usize,
}

impl Endpoint for OpsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        let mut path = format!(
            "/api/portal/rust-ui/opps?entity={}&page={}&search={}",
            encode(&self.entity),
            self.page,
            encode(&self.search)
        );
        if let Some(selected) = self.selected.as_deref().filter(|id| !id.is_empty()) {
            path.push_str(&format!("&selected={}", encode(selected)));
        }
        path
    }
}

/// One Workbench write (`save`, `createProperty`). Answers the refreshed Workbench page.
pub struct OpsCommand {
    pub body: serde_json::Value,
}

impl Endpoint for OpsCommand {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/opps".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

/// Where property photographs are sent, in pieces (`Cmd::upload`).
pub const PROPERTY_MEDIA_CHUNKED: &str = "/api/property-media/chunked";

/// Listing Media: listings with their photo counts, one opened when `selected` is set. Answers `{ listingMedia: ... }`.
pub struct ListingMediaRead {
    pub selected: Option<String>,
    pub search: String,
    /// 0-based.
    pub page: usize,
}

impl Endpoint for ListingMediaRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        let mut path = format!(
            "/api/portal/rust-ui/listing-media?page={}&search={}",
            self.page,
            encode(&self.search)
        );
        if let Some(selected) = self.selected.as_deref().filter(|id| !id.is_empty()) {
            path.push_str(&format!("&selected={}", encode(selected)));
        }
        path
    }
}

/// A public page's content (hero, blocks, listings, guide, FAQ), as the site reads it. `scope` is the record a page is
/// about (a property's slug). Answers the page itself.
pub struct PublicPage {
    pub screen: &'static str,
    pub scope: Option<String>,
}

impl Endpoint for PublicPage {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PageContent;
    fn path(&self) -> String {
        match self.scope.as_deref().filter(|scope| !scope.is_empty()) {
            Some(scope) => format!(
                "/api/rust-ui/public-page?screen={}&scope={}",
                self.screen,
                encode(scope)
            ),
            None => format!("/api/rust-ui/public-page?screen={}", self.screen),
        }
    }
}

/// A website lead (contact form, quick enquiries). The relay hands it to the one intake pipeline. Answers
/// `{ accepted, status }`.
pub struct WebsiteIntake {
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct IntakeAnswer {
    pub accepted: bool,
}

impl Endpoint for WebsiteIntake {
    const METHOD: Method = Method::Post;
    type Response = IntakeAnswer;
    fn path(&self) -> String {
        "/api/rust-ui/website-intake".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}
