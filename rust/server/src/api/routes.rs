use super::context::{
    asserted_identity_context, resolve_public_guest_context, resolve_request_context,
    ResolvedRequestContext,
};
use super::{diagnostics, engine, ApiError, ApiState};
use crate::service_support::CoreServiceError;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use domain::{
    AttachPropertyVideoRequest, ClientAdminPageRequest, ClientDirectoryPageRequest,
    ClientHistoryRequest, GetCommsPanelRequest, GetCommsTimelineRequest, SearchPeopleRequest,
    UploadPropertyMediaRequest, VaultActorScope, MAX_MEDIA_UPLOAD_BYTES,
};
mod accounting_vault;
mod bodies;
mod deals_forms_comms;
mod media;
mod people_properties;
mod public;
mod security_service;
mod webhooks_support;
mod work;
#[allow(unused_imports)]
pub(super) use accounting_vault::*;
#[allow(unused_imports)]
pub(super) use bodies::*;
#[allow(unused_imports)]
pub(super) use deals_forms_comms::*;
#[allow(unused_imports)]
pub(super) use media::*;
#[allow(unused_imports)]
pub(super) use people_properties::*;
#[allow(unused_imports)]
pub(super) use public::*;
#[allow(unused_imports)]
pub(super) use security_service::*;
#[allow(unused_imports)]
pub(super) use webhooks_support::*;
#[allow(unused_imports)]
pub(super) use work::*;

#[cfg(test)]
use domain::{VaultArtifactFailure, VaultCommandOutcome};
use integrations::mux::{MuxClient, MuxConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use service::{
    CommandRequest, CommandResult, OperationKind, ServiceContext, ServiceControlCommand,
    ServiceControlResult, ServiceDispatchError, ServiceEnvelope,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiSuccess<T> {
    ok: bool,
    value: T,
    correlation_id: String,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .merge(super::portal_bridge::router())
        .merge(super::google_auth::router())
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/v1/whoami", get(whoami))
        // ABSTRACT SERVICE INGRESS. The transport resolves identity/context; callers supply only
        // domain + operation + payload. Typed services remain authoritative underneath.
        .route("/v1/services", get(service_catalog))
        .route("/v1/services/health", get(service_health))
        .route("/v1/services/kernel/health", get(service_kernel_health))
        .route("/v1/services/runtime/health", get(service_runtime_health))
        .route("/v1/services/{domain}/control", post(service_control))
        .route("/v1/services/dispatch", post(service_dispatch))
        .route("/v1/commands/dispatch", post(command_dispatch))
        // THE LOGIN SEAM'S QUESTION, as opposed to whoami's. Auth.js has proved a Google subject and nobody
        // knows yet whether it maps to an active application user; this answers known / unmapped / inactive.
        .route("/v1/security/identity", get(security_identity))
        .route("/v1/diagnostics/app-error", post(record_app_diagnostic))
        .route(
            "/api/integrations/whatsapp/webhook",
            get(whatsapp_handshake).post(whatsapp_webhook),
        )
        .route("/v1/security/guests", post(provision_guest))
        .route("/v1/security/guest-code", post(request_guest_code))
        .route("/v1/security/guest-code/verify", post(verify_guest_code))
        // THE SINGLE DECISION SURFACE: the TypeScript kernel asks here rather than holding its own copy of the
        // rule, so "may this principal do this" is answered in one place.
        .route("/v1/security/authorize", post(authorize_action))
        // The same decision for the anonymous public site, which has no principal to decide for. What it may
        // reach is the policy's named public-read list, not this route's business.
        .route(
            "/v1/security/authorize/public",
            post(authorize_public_action),
        )
        .route(
            "/v1/security/role-entitlements",
            get(role_entitlements).put(set_role_entitlement),
        )
        .route(
            "/v1/security/users",
            get(security_users).put(set_user_primary_role),
        )
        .route("/v1/cockpit", get(cockpit))
        .route("/v1/tech/cockpit", get(tech_cockpit).post(tech_command))
        .route("/v1/support/security-status", get(support_security_status))
        .route(
            "/v1/support/break-glass-readiness",
            get(support_break_glass_readiness),
        )
        .route("/v1/support/system-health", get(support_system_health))
        .route(
            "/v1/support/workflow-diagnostics",
            get(support_workflow_diagnostics),
        )
        .route(
            "/v1/support/workflow-diagnostics/{id}",
            get(support_workflow_detail),
        )
        .route("/v1/workflows", get(workflows))
        .route("/v1/workflows/{id}", get(workflow_detail))
        .route("/v1/flight-recorder/{id}", get(flight_recorder))
        .route("/v1/projects", get(projects).post(create_project))
        .route("/v1/projects/{id}", get(project).patch(update_project))
        .route("/v1/wbs", post(create_wbs_item))
        .route("/v1/wbs/project-items", get(wbs_project_items))
        .route(
            "/v1/wbs/dependencies/{project_id}",
            get(wbs_dependencies).post(add_wbs_dependency),
        )
        .route(
            "/v1/wbs/dependencies/{project_id}/{source_id}/{target_id}",
            axum::routing::delete(remove_wbs_dependency),
        )
        .route("/v1/wbs/{id}", get(wbs_item).patch(update_wbs_item))
        .route("/v1/tasks/{id}/complete", post(complete_task))
        .route("/v1/wbs/{id}/apple-reminder", post(queue_apple_reminder))
        .route("/v1/wbs/{id}/route", post(route_project_work))
        .route("/v1/clients", get(clients))
        .route("/v1/clients/agents", get(client_agents))
        .route("/v1/clients/{person_id}/history", get(client_history))
        .route("/v1/clients/{person_id}", get(client_detail))
        .route("/v1/people/search", get(search_people))
        .route("/v1/people/{id}", get(person).patch(update_person_admin))
        .route("/v1/people/{id}/properties", get(properties_for_person))
        .route(
            "/v1/properties/admin",
            get(property_admin_page).post(create_property_admin),
        )
        .route(
            "/v1/properties/{id}/admin",
            get(property_admin_detail).patch(save_property_admin),
        )
        .route("/v1/properties/{id}", get(property))
        .route("/v1/media/{id}", get(private_media))
        .route(
            "/v1/media/upload",
            post(upload_standalone_media)
                .layer(DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route(
            "/v1/properties/{id}/media",
            get(property_media)
                .post(upload_property_media)
                .layer(DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route("/v1/properties/{id}/video", post(attach_property_video))
        // The chunked upload, for the photographs too large for one request. The per-chunk limit is deliberately a
        // few megabytes rather than the whole-file limit: a chunk is a fragment by definition, and the gateway
        // refuses anything past ~4.5 MB anyway.
        .route(
            "/v1/properties/{id}/media/uploads",
            post(begin_media_upload),
        )
        .route(
            "/v1/properties/{id}/media/uploads/{upload_id}/chunks/{index}",
            post(stage_media_chunk).layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/v1/properties/{id}/media/uploads/{upload_id}/complete",
            post(complete_media_upload),
        )
        .route(
            "/v1/properties/{id}/video-uploads",
            post(create_property_video_upload),
        )
        .route(
            "/v1/properties/{id}/video-uploads/{upload_id}/finalize",
            post(finalize_property_video_upload),
        )
        .route("/v1/deals", get(deals).post(create_deal))
        .route("/v1/deals/{id}", get(deal_workspace))
        .route("/v1/deals/{id}/commands", post(deal_workspace_command))
        .route("/v1/contracts", get(contracts))
        .route("/v1/contracts/{id}", get(contract))
        .route(
            "/v1/process-instances/{id}/contracts",
            get(contracts_for_process_instance),
        )
        .route("/v1/forms", get(forms).post(create_form))
        .route("/v1/forms/deal-facts/{deal_id}", get(form_deal_facts))
        .route("/v1/forms/{id}/signers", get(form_signers))
        .route("/v1/forms/{id}/issued-document", get(form_issued_document))
        .route("/v1/forms/{id}", get(form).patch(update_form))
        .route("/v1/comms/{person_id}/panel", get(comms_panel))
        .route("/v1/comms/{person_id}/timeline", get(comms_timeline))
        .route("/v1/activity", get(activity))
        .route("/v1/issues", get(issues))
        .route(
            "/v1/relationship-evidence/review",
            get(relationship_evidence_review),
        )
        .route(
            "/v1/relationship-evidence/actions",
            post(relationship_evidence_action),
        )
        // Accounting V1: the two canonical tables, read as lists and as the projections over them, plus the three
        // commands. The P&L takes its period from the query string — the range is the caller's, and a route that invented
        // one would be the reason a filter could not be honoured.
        .route("/v1/accounting/dashboard", get(accounting_dashboard))
        .route(
            "/v1/accounting/receivables",
            get(accounting_receivables).post(create_receivable),
        )
        .route(
            "/v1/accounting/receivables/{id}/paid",
            post(mark_receivable_paid),
        )
        .route(
            "/v1/accounting/expenses",
            get(accounting_expenses).post(create_expense),
        )
        .route(
            "/v1/accounting/expense-categories",
            get(accounting_expense_categories),
        )
        .route("/v1/accounting/pnl", get(accounting_pnl))
        .route(
            "/v1/calendar",
            get(calendar).post(create_apple_calendar_event),
        )
        .route("/v1/vault/documents", get(vault_documents))
        .route("/v1/vault/documents/{id}", get(vault_document))
        .route(
            "/v1/vault/deals/{id}/documents",
            get(vault_documents_by_deal),
        )
        .route("/v1/vault/forms/{id}/contract", get(vault_form_contract))
        .route(
            "/v1/vault/forms/{id}/bind-contract",
            post(vault_bind_form_contract),
        )
        .route(
            "/v1/vault/contracts/{contract_id}/templates/{template_id}/prior",
            get(vault_prior_contract_document),
        )
        .route(
            "/v1/vault/public-listing-documents/{id}",
            get(vault_public_listing_document_bytes),
        )
        .route("/v1/public/listing-copy", get(public_listing_copy))
        // THE PUBLIC INVENTORY, served by the Rust service the way every other read is. The site's own thin proxy
        // shapes this for the buyers grid, so the grid stops reaching into the database itself.
        .route("/v1/public/listings", get(public_listings))
        // ONE PROPERTY, by any key that names it: slug, name, or id. The property page reads this.
        .route("/v1/public/property", get(public_property))
        // ONE PHOTOGRAPH'S BYTES. The web copy when there is one, the original otherwise — the site never sends a
        // 13 MB original through a 4.5 MB gateway, and never reads Postgres itself to find out.
        .route("/v1/public/media/{id}", get(public_media))
        // THE PROPERTY PAGE'S STRIP, and the sitemap's list. Both by the same rule as the inventory.
        .route("/v1/public/similar", get(public_similar))
        .route("/v1/public/slugs", get(public_slugs))
        .route("/v1/public/guide", get(public_guide))
        .route(
            "/v1/public/marketing-content",
            get(public_marketing_content),
        )
        .route("/v1/website-intake", post(submit_website_intake))
        .route("/v1/catchup/leads", post(submit_catchup_lead))
        .route("/v1/website-intake/{id}/notify", post(notify_website_lead))
        .route(
            "/v1/vault/document-bytes/{id}",
            get(vault_private_document_bytes),
        )
        // The workflow engine, served. Same verbs the re-workflow CLI accepted, now behind the internal key and the
        // same identity resolution as every other route, so an engine command is a first-class part of this server
        // instead of a spawned process with its own pool and no error capture.
        .route("/v1/diagnostics/db", get(diagnostics::db_metrics))
        .route("/v1/engine/transactions", post(engine::start_transaction))
        .route("/v1/engine/timers/reconcile", post(engine::reconcile_timer))
        .route("/v1/engine/tasks/complete", post(engine::complete_task))
        .route("/v1/engine/reclaim", post(engine::reclaim))
        // Signature (BoldSign). Mirrors the production endpoints the TypeScript path already serves, so the webhook
        // and the operator actions can be pointed at Rust without changing a client contract.
        .route("/v1/signature/requests", post(signature_send))
        .route("/v1/signature/requests/{id}", get(signature_request))
        .route(
            "/v1/signature/requests/{id}/refresh",
            post(signature_refresh),
        )
        .route(
            "/api/integrations/boldsign/webhook",
            post(signature_webhook),
        )
        .merge(super::public_ui::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            registered_service_mailbox,
        ))
        // Everything the API does not claim is the website: static files, else the Yew shell (crate::site).
        .fallback_service(crate::site::service(state.clone()))
        .with_state(state)
}

async fn registered_service_mailbox(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let Some(domain) = http_service_domain(&path) else {
        return next.run(request).await;
    };
    let method = request.method().to_string();
    let operation = format!("http.{}", method.to_ascii_lowercase());
    let payload = json!({ "method": method, "path": path });
    match state
        .service_gateway()
        .execute(domain, &operation, &payload, async move {
            next.run(request).await
        })
        .await
    {
        Ok(response) => response,
        Err(error) => service_dispatch_error(error).into_response(),
    }
}

fn http_service_domain(path: &str) -> Option<&'static str> {
    if path == "/api/portal/rust-ui/clients" {
        return Some("client");
    }
    if path == "/v1/cockpit"
        || path.starts_with("/v1/workflows")
        || path.starts_with("/v1/flight-recorder")
        || path.starts_with("/v1/tasks/")
    {
        return None;
    }
    if path == "/api/integrations/whatsapp/webhook" {
        return Some("whatsapp");
    }
    if path == "/api/integrations/boldsign/webhook" || path.starts_with("/v1/signature/") {
        return Some("signature");
    }
    if path.starts_with("/v1/security/") {
        return Some(if path.contains("guest") {
            "guest-sign-in"
        } else {
            "security"
        });
    }
    if path.starts_with("/v1/support/") {
        return Some("support");
    }
    if path.starts_with("/v1/tech/") {
        return Some("tech");
    }
    if path.starts_with("/v1/projects") {
        return Some("project");
    }
    if path.starts_with("/v1/wbs") {
        return Some("wbs");
    }
    if path.starts_with("/v1/clients") {
        return Some("client");
    }
    if path.starts_with("/v1/people/") && path.ends_with("/properties") {
        return Some("property");
    }
    if path.starts_with("/v1/people") {
        return Some("person");
    }
    if path.starts_with("/v1/properties/") && (path.contains("/media") || path.contains("/video")) {
        return Some("media");
    }
    if path.starts_with("/v1/properties") {
        return Some("property");
    }
    if path.starts_with("/v1/media") {
        return Some("media");
    }
    if path.starts_with("/v1/deals") {
        return Some("deal");
    }
    if path.starts_with("/v1/contracts") || path.contains("/contracts") {
        return Some("contract");
    }
    if path.contains("/issued-document") || path.starts_with("/v1/vault") {
        return Some("vault");
    }
    if path.starts_with("/v1/forms") {
        return Some("forms");
    }
    if path.starts_with("/v1/comms") || path == "/v1/activity" {
        return Some("communications");
    }
    if path == "/v1/issues" {
        return Some("issue");
    }
    if path.starts_with("/v1/relationship-evidence") {
        return Some("relationship-evidence");
    }
    if path.starts_with("/v1/accounting") {
        return Some("accounting");
    }
    if path.starts_with("/v1/calendar") {
        return Some("calendar");
    }
    if path.starts_with("/v1/public/listing")
        || path.starts_with("/v1/public/property")
        || path.starts_with("/v1/public/media")
        || path.starts_with("/v1/public/similar")
        || path.starts_with("/v1/public/slugs")
    {
        return Some("public-listing");
    }
    if path == "/v1/public/guide" {
        return Some("guide");
    }
    if path == "/v1/public/marketing-content" {
        return Some("marketing");
    }
    if path == "/v1/website-intake" || path == "/v1/catchup/leads" {
        return Some("intake");
    }
    if path.starts_with("/v1/website-intake/") {
        return Some("website-lead");
    }
    None
}

pub(crate) fn success<T>(value: T, resolved: &ResolvedRequestContext) -> Json<ApiSuccess<T>> {
    Json(ApiSuccess {
        ok: true,
        value,
        correlation_id: resolved.service.correlation_id.clone(),
    })
}

/// The same envelope for callers that have a correlation id but not a full resolved request context - an engine command
/// from a background job has no acting user, and inventing one would be worse than admitting there is not one.
pub(crate) fn success_with_correlation<T>(value: T, correlation_id: &str) -> Json<ApiSuccess<T>> {
    Json(ApiSuccess {
        ok: true,
        value,
        correlation_id: correlation_id.to_owned(),
    })
}

fn correlate(error: ApiError, resolved: &ResolvedRequestContext) -> ApiError {
    error.with_correlation(resolved.service.correlation_id.clone())
}

pub(super) async fn execute_registered<T, F>(
    state: &ApiState,
    domain: &str,
    operation: &str,
    payload: serde_json::Value,
    work: F,
) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, CoreServiceError>> + Send + 'static,
{
    state
        .service_gateway()
        .execute(domain, operation, &payload, work)
        .await
        .map_err(service_dispatch_error)?
        .map_err(ApiError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_failure_for_vault_renderer_is_explicit() {
        let failure = VaultArtifactFailure {
            outcome: VaultCommandOutcome::PreconditionFailure,
            message: "not configured".into(),
        };
        assert_eq!(failure.outcome, VaultCommandOutcome::PreconditionFailure);
    }

    #[test]
    fn api_success_shape_is_camel_case() {
        let body = ApiSuccess {
            ok: true,
            value: json!({"hello": "world"}),
            correlation_id: "corr-1".into(),
        };
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(value["correlationId"], "corr-1");
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn every_http_service_family_maps_to_its_registered_mailbox() {
        for (path, domain) in [
            ("/api/portal/rust-ui/clients", "client"),
            ("/api/integrations/whatsapp/webhook", "whatsapp"),
            ("/api/integrations/boldsign/webhook", "signature"),
            ("/v1/security/guest-code", "guest-sign-in"),
            ("/v1/security/users", "security"),
            ("/v1/support/system-health", "support"),
            ("/v1/tech/cockpit", "tech"),
            ("/v1/projects", "project"),
            ("/v1/wbs/project-items", "wbs"),
            ("/v1/clients", "client"),
            ("/v1/people/search", "person"),
            ("/v1/people/x/properties", "property"),
            ("/v1/properties/admin", "property"),
            ("/v1/properties/x/media/uploads", "media"),
            ("/v1/media/upload", "media"),
            ("/v1/deals", "deal"),
            ("/v1/contracts", "contract"),
            ("/v1/forms/x/issued-document", "vault"),
            ("/v1/forms", "forms"),
            ("/v1/comms/x/panel", "communications"),
            ("/v1/issues", "issue"),
            ("/v1/relationship-evidence/review", "relationship-evidence"),
            ("/v1/accounting/dashboard", "accounting"),
            ("/v1/calendar", "calendar"),
            ("/v1/public/listings", "public-listing"),
            ("/v1/public/guide", "guide"),
            ("/v1/public/marketing-content", "marketing"),
            ("/v1/website-intake", "intake"),
            ("/v1/website-intake/x/notify", "website-lead"),
        ] {
            assert_eq!(http_service_domain(path), Some(domain), "{path}");
        }
    }
}
