//! Portal endpoints: security grants and roles, rows, accounting, publishing, catch-up, cockpit, cabinet, deals, tech.

#[allow(unused_imports)]
use super::*;

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
    pub(super) body: serde_json::Value,
}

impl CatchUpAction {
    pub fn handle(person_id: String, reason_code: String) -> Self {
        Self {
            body: serde_json::json!({
                "action": "handle",
                "personId": person_id,
                "reasonCode": reason_code
            }),
        }
    }

    pub fn snooze(person_id: String, reason_code: String, days: i32) -> Self {
        Self {
            body: serde_json::json!({
                "action": "snooze",
                "personId": person_id,
                "reasonCode": reason_code,
                "days": days
            }),
        }
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

/// One process instance's Flight Recorder trace, read for the TECH console.
///
/// The internal `/v1/flight-recorder/{id}` is a machine API a page cannot reach; this is the portal
/// address that resolves the caller once and returns the same transaction read-model, camelCase, unwrapped.
pub struct FlightRecorderRead {
    pub instance_id: String,
}

impl Endpoint for FlightRecorderRead {
    const METHOD: Method = Method::Get;
    type Response = model::FlightRecorderTransaction;
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

/// The ops signing desk: recent native envelopes through the generic service
/// dispatch, so the desk needs no bespoke read route. Answers the summaries.
pub struct SigningDeskList;

impl Endpoint for SigningDeskList {
    const METHOD: Method = Method::Post;
    type Response = Vec<crate::model::SigningEnvelopeSummary>;
    fn path(&self) -> String {
        "/v1/services/dispatch".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "domain": "document-sign",
            "operation": "documentSign.list",
            "payload": {},
        }))
    }
}

/// One envelope's detail (config, recipients with live states, fields).
/// Answers the snapshot value.
pub struct SigningEnvelopeGet {
    pub signature_request_id: String,
}

impl Endpoint for SigningEnvelopeGet {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/v1/services/dispatch".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "domain": "document-sign",
            "operation": "documentSign.get",
            "payload": { "signatureRequestId": self.signature_request_id },
        }))
    }
}

/// One durable signing command from the desk (resend, void): the envelope
/// the generic command dispatcher executes. Answers the command result.
pub struct SigningDeskCommand {
    pub command_id: String,
    pub command_type: &'static str,
    /// `signature_request` for everything after prepare; `transaction_document` for prepare itself.
    pub aggregate_type: &'static str,
    pub aggregate_id: String,
    pub requested_at: String,
    pub input: serde_json::Value,
}

impl SigningDeskCommand {
    /// A command on an existing envelope.
    pub fn on_envelope(
        command_id: String,
        command_type: &'static str,
        signature_request_id: String,
        requested_at: String,
        input: serde_json::Value,
    ) -> Self {
        Self {
            command_id,
            command_type,
            aggregate_type: "signature_request",
            aggregate_id: signature_request_id,
            requested_at,
            input,
        }
    }

    /// `documentSign.prepare`: the envelope does not exist yet, so the aggregate is the document.
    pub fn prepare(
        command_id: String,
        transaction_document_id: String,
        requested_at: String,
        input: serde_json::Value,
    ) -> Self {
        Self {
            command_id,
            command_type: "documentSign.prepare",
            aggregate_type: "transaction_document",
            aggregate_id: transaction_document_id,
            requested_at,
            input,
        }
    }
}

impl Endpoint for SigningDeskCommand {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/v1/commands/dispatch".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        let mut input = self.input.as_object().cloned().unwrap_or_default();
        if self.aggregate_type == "signature_request" {
            input.insert(
                "signatureRequestId".into(),
                serde_json::Value::String(self.aggregate_id.clone()),
            );
        } else {
            input.insert(
                "transactionDocumentId".into(),
                serde_json::Value::String(self.aggregate_id.clone()),
            );
        }
        Some(serde_json::json!({
            "commandId": self.command_id,
            "commandType": self.command_type,
            "aggregateType": self.aggregate_type,
            "aggregateId": self.aggregate_id,
            "requestedAt": self.requested_at,
            "input": input,
        }))
    }
}

/// The documents the desk can send for signature: the Vault's issued documents (`GET /v1/vault/documents`).
pub struct SigningDocumentsList;

impl Endpoint for SigningDocumentsList {
    const METHOD: Method = Method::Get;
    type Response = Vec<crate::model::SigningDocumentOption>;
    fn path(&self) -> String {
        "/v1/vault/documents".into()
    }
}
