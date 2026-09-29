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

/// The signed-in external account's own transaction room.
pub struct ClientRoomRead;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientRoomResponse {
    pub linked: bool,
    pub room: Option<crate::model::PortalClientRoom>,
}

impl Endpoint for ClientRoomRead {
    const METHOD: Method = Method::Get;
    type Response = ClientRoomResponse;
    fn path(&self) -> String {
        "/api/rust-ui/client-room".into()
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

// ---------------------------------------------------------------- Forms

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FormsBridgeResponse {
    pub forms: FormsPage,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsPage {
    pub items: Vec<FormItem>,
    pub selected: Option<FormItem>,
    pub template: Option<FormTemplate>,
    pub issued: Option<FormIssuedDocument>,
    pub signers: Vec<FormSigner>,
    pub signature: Option<FormSignatureState>,
    pub template_choices: Vec<FormTemplateChoice>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormItem {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateChoice {
    pub id: String,
    pub display_name: String,
    pub active_version: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplate {
    pub id: String,
    pub version: i32,
    pub active_version: i32,
    pub display_name: String,
    pub document_type_label: String,
    pub rendering_title: String,
    pub presentation: String,
    pub fields: Vec<FormTemplateField>,
    pub sections: Vec<FormTemplateSection>,
    pub signature_groups: Vec<FormSignatureGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateField {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    pub options: Vec<String>,
    pub when: Option<FormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormWhen {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateSection {
    pub name: String,
    pub label: String,
    pub editable: bool,
    pub segments: Vec<FormTemplateSegment>,
    pub when: Option<FormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateSegment {
    pub kind: String,
    pub text: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSignatureGroup {
    pub role: String,
    pub label: String,
    pub field: Option<String>,
    pub initials: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSigner {
    pub slot_id: Option<String>,
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSignatureState {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormIssuedDocument {
    pub document_id: String,
    pub issued_version: i32,
    pub checksum: String,
    pub created_at: String,
    pub media_id: Option<String>,
}

pub struct FormsRead {
    pub record: Option<String>,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
}

impl FormsRead {
    pub fn list(
        deal_id: Option<String>,
        person_id: Option<String>,
        property_id: Option<String>,
    ) -> Self {
        Self {
            record: None,
            deal_id,
            person_id,
            property_id,
        }
    }

    pub fn record(id: impl Into<String>) -> Self {
        Self {
            record: Some(id.into()),
            deal_id: None,
            person_id: None,
            property_id: None,
        }
    }
}

impl Endpoint for FormsRead {
    const METHOD: Method = Method::Get;
    type Response = FormsBridgeResponse;

    fn path(&self) -> String {
        if let Some(id) = &self.record {
            return format!(
                "/api/portal/rust-ui/forms?screen=form-record&scope={}",
                encode(id)
            );
        }
        let mut path = "/api/portal/rust-ui/forms?screen=forms".to_string();
        if let Some(deal_id) = &self.deal_id {
            path.push_str(&format!("&dealId={}", encode(deal_id)));
        }
        if let Some(person_id) = &self.person_id {
            path.push_str(&format!("&personId={}", encode(person_id)));
        }
        if let Some(property_id) = &self.property_id {
            path.push_str(&format!("&propertyId={}", encode(property_id)));
        }
        path
    }
}

pub enum FormsAction {
    Create {
        template_id: String,
        deal_id: Option<String>,
        person_id: Option<String>,
        property_id: Option<String>,
        /// The seller as named on the contract: the server finds that person (or makes them).
        seller_name: Option<String>,
        /// The property's catastro number: the server finds that property.
        catastro: Option<String>,
    },
    Save {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
    Issue {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
    FillClient {
        form_id: String,
        seller_name: String,
    },
    SendSignature {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
}

/// Grok's suggestion for the open form: the fields it sets, optionally new document prose, and a note. Nothing is saved.
pub struct FormsGrok {
    pub form_id: String,
    pub form_name: String,
    pub prompt: String,
    pub details_text: String,
    pub field_values: std::collections::BTreeMap<String, String>,
    pub fields: Vec<FormTemplateField>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsGrokAnswer {
    pub field_values: std::collections::BTreeMap<String, String>,
    pub body: Option<String>,
    pub note: String,
}

impl Endpoint for FormsGrok {
    const METHOD: Method = Method::Post;
    type Response = FormsGrokAnswer;
    fn path(&self) -> String {
        "/api/portal/rust-ui/forms/grok".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        let fields: Vec<serde_json::Value> = self
            .fields
            .iter()
            .map(|field| serde_json::json!({ "name": field.name, "label": field.label, "type": field.field_type, "options": field.options }))
            .collect();
        Some(serde_json::json!({
            "formId": self.form_id,
            "formName": self.form_name,
            "prompt": self.prompt,
            "detailsText": self.details_text,
            "fieldValues": self.field_values,
            "fields": fields,
        }))
    }
}

pub struct FormsWrite {
    pub action: FormsAction,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsWriteResponse {
    pub form_id: String,
    pub forms: FormsPage,
    pub message: Option<String>,
}

impl Endpoint for FormsWrite {
    const METHOD: Method = Method::Post;
    type Response = FormsWriteResponse;

    fn path(&self) -> String {
        "/api/portal/rust-ui/forms".into()
    }

    fn body(&self) -> Option<serde_json::Value> {
        Some(match &self.action {
            FormsAction::Create {
                template_id,
                deal_id,
                person_id,
                property_id,
                seller_name,
                catastro,
            } => serde_json::json!({
                "action": "create",
                "templateId": template_id,
                "dealId": deal_id,
                "personId": person_id,
                "propertyId": property_id,
                "sellerName": seller_name,
                "catastro": catastro,
            }),
            FormsAction::Save {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "save",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
            FormsAction::Issue {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "issue",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
            FormsAction::FillClient {
                form_id,
                seller_name,
            } => serde_json::json!({
                "action": "fillClient",
                "formId": form_id,
                "sellerName": seller_name,
            }),
            FormsAction::SendSignature {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "sendSignature",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormPreviewResponse {
    pub data_uri: String,
    pub filename: String,
}

pub struct FormPreview {
    pub form_id: String,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
}

impl Endpoint for FormPreview {
    const METHOD: Method = Method::Post;
    type Response = FormPreviewResponse;

    fn path(&self) -> String {
        "/api/portal/rust-ui/forms/preview".into()
    }

    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "formId": self.form_id,
            "fieldValues": self.field_values,
            "sections": self.sections,
        }))
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

/// Listing publication readiness. Answers `{ publishing: ... }`.
pub struct PublishingRead;

impl Endpoint for PublishingRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/publishing".into()
    }
}

/// Relationship Catch-Up queue. Answers `{ catchUp: ... }`.
pub struct CatchUpRead;

impl Endpoint for CatchUpRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/catch-up".into()
    }
}

pub struct CatchUpAction {
    body: serde_json::Value,
}

impl CatchUpAction {
    pub fn handle(person_id: String, reason_code: String) -> Self {
        Self { body: serde_json::json!({
            "action": "handle",
            "personId": person_id,
            "reasonCode": reason_code
        }) }
    }

    pub fn snooze(person_id: String, reason_code: String, days: i32) -> Self {
        Self { body: serde_json::json!({
            "action": "snooze",
            "personId": person_id,
            "reasonCode": reason_code,
            "days": days
        }) }
    }
}

impl Endpoint for CatchUpAction {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/catch-up".into()
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectsCalendarViewportResponse {
    pub calendar: Vec<crate::model::PortalProjectCalendarEvent>,
}

pub struct ProjectsCalendarRead {
    pub start_at: String,
    pub end_at: String,
}

impl Endpoint for ProjectsCalendarRead {
    const METHOD: Method = Method::Get;
    type Response = ProjectsCalendarViewportResponse;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/projects/calendar?startAt={}&endAt={}",
            encode(&self.start_at),
            encode(&self.end_at)
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarCommandReceipt {
    pub command_id: String,
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarCommandState {
    pub command_id: String,
    pub state: String,
    pub delivered_at: Option<String>,
    pub reconciled_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct ProjectsCalendarUpdate {
    pub event_id: String,
    pub calendar_item_id: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub recurrence_scope: String,
}

impl Endpoint for ProjectsCalendarUpdate {
    const METHOD: Method = Method::Post;
    type Response = CalendarCommandReceipt;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects/calendar".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "eventId": self.event_id,
            "calendarItemId": self.calendar_item_id,
            "startAt": self.start_at,
            "endAt": self.end_at,
            "allDay": self.all_day,
            "recurrenceScope": self.recurrence_scope,
        }))
    }
}

pub struct ProjectsCalendarCommandState {
    pub command_id: String,
}

impl Endpoint for ProjectsCalendarCommandState {
    const METHOD: Method = Method::Get;
    type Response = CalendarCommandState;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/projects/calendar-command?commandId={}",
            encode(&self.command_id)
        )
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

/// Sign-in and sign-out are full-page form posts (the server sets or clears the session cookie and redirects), not
/// fetches — but their URLs still live here, with every other one.
pub mod auth {
    pub const SIGN_OUT: &str = "/api/auth/signout";
    pub const SIGN_IN_GOOGLE: &str = "/api/auth/signin/google";
    pub const EMAIL_CODE_CALLBACK: &str = "/api/auth/callback/email-code";
    pub const BREAK_GLASS_CALLBACK: &str = "/api/auth/callback/break-glass";

    /// Google sign-in, returning to `back` (a portal path) afterwards.
    pub fn sign_in_google(back: &str) -> String {
        format!("{SIGN_IN_GOOGLE}?callbackUrl={}", encode(back))
    }

    fn encode(text: &str) -> String {
        text.bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (byte as char).to_string(),
                _ => format!("%{byte:02X}"),
            })
            .collect()
    }
}

/// Addresses a page LINKS to rather than fetches — an image's `src`, a document's download `href`, a form's `action`.
/// They are URLs all the same, so they live here.
pub mod links {
    /// A stored photograph (`?size=card|thumb` picks a derived copy; none is the web copy).
    pub fn media(media_id: &str) -> String {
        format!("/api/media/{media_id}")
    }

    /// A property document for the public site.
    pub fn property_document(document_id: &str) -> String {
        format!("/api/media/documents/{document_id}")
    }

    /// A Vault document's PDF: the executed copy when `signed`, the issued one otherwise.
    pub fn vault_document(document_id: &str, signed: bool) -> String {
        format!("/api/portal/documents/{document_id}/file{}", if signed { "?artifact=signed" } else { "" })
    }

    /// A Vault document's signature audit trail.
    pub fn vault_audit(document_id: &str) -> String {
        format!("/api/portal/documents/{document_id}/file?artifact=audit")
    }

    /// The Seller Strategy PDF (a form post).
    pub const SELLER_STRATEGY_PDF: &str = "/api/portal/seller-strategy/pdf";
}

/// Where property photographs are sent, in pieces (`Cmd::upload`).
pub struct PropertyMediaChunked;

impl FileEndpoint for PropertyMediaChunked {
    fn path(&self) -> String {
        "/api/property-media/chunked".into()
    }
}

/// A project document's signed copy: the PDF and the date it was signed (`Cmd::post_form`).
pub struct ProjectSignedCopy {
    pub document_id: String,
}

impl FileEndpoint for ProjectSignedCopy {
    fn path(&self) -> String {
        format!("/api/portal/projects/documents/{}/signed", self.document_id)
    }
}

/// Make one of a property's photographs its hero (the previous hero returns to the gallery).
pub struct PropertyHero {
    pub property_id: String,
    pub media_id: String,
}

impl Endpoint for PropertyHero {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/property-media/hero".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "mediaId": self.media_id }))
    }
}

/// FIND by catastro: the other record for that parcel is merged into this property (see the server's
/// `merge_parcel_record`). Answers `{ merged, mergedName }`.
pub struct PropertyMergeParcel {
    pub property_id: String,
    pub catastro: String,
}

impl Endpoint for PropertyMergeParcel {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/portal/property/merge-parcel".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "catastro": self.catastro }))
    }
}

/// A contract known to be signed, its PDF still to come: recorded as sent, and the project's signing step done.
pub struct ProjectDocumentSignedCopyToCome {
    pub document_id: String,
    pub project_id: String,
    pub signed_at: String,
}

impl Endpoint for ProjectDocumentSignedCopyToCome {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        format!("/api/portal/projects/documents/{}/signed-copy-to-come", self.document_id)
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "projectId": self.project_id, "signedAt": self.signed_at }))
    }
}

/// Take a photograph off a property (deleted with its copies unless another property shows it).
pub struct PropertyMediaRemove {
    pub property_id: String,
    pub media_id: String,
}

impl Endpoint for PropertyMediaRemove {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/property-media/remove".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "mediaId": self.media_id }))
    }
}

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
