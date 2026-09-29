//! THE PORTAL'S OWN ADDRESSES — every endpoint the Yew portal calls (`rust/ui/src/app/api.rs`), answered by the services.
//!
//! These replace server-side TypeScript relays. A request resolves its caller
//! once, calls the long-lived Rust services directly, and returns the payload
//! shape consumed by the Rust UI.

use super::context::ResolvedRequestContext;
use super::ui_auth::resolve_portal_context;
use super::{ApiError, ApiState};
use crate::service_support::CoreServiceError;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use base64::Engine as _;
use chrono::Utc;
use domain::{
    ClientDetail, ClientDirectoryPageRequest, ClientsPageResult, CommsPanel, GetCommsPanelRequest,
    PersonPropertyContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::routes::{
    apply_person_admin_update, apply_project_update, apply_property_admin_create, apply_property_admin_save,
    apply_wbs_update, break_glass_readiness, execute_registered, CreatePropertyAdminBody, SavePropertyAdminBody, UpdatePersonAdminBody,
    UpdateProjectBody, UpdateWbsBody,
};
mod clients;
mod pages;
mod forms_templates;
mod forms_values;
mod forms_write;
mod projects;
mod accounting_opps;
mod security_media;
mod workspaces;
mod tech_documents;
#[allow(unused_imports)]
pub(super) use clients::*;
#[allow(unused_imports)]
pub(super) use pages::*;
#[allow(unused_imports)]
pub(super) use forms_templates::*;
#[allow(unused_imports)]
pub(super) use forms_values::*;
#[allow(unused_imports)]
pub(super) use forms_write::*;
#[allow(unused_imports)]
pub(super) use projects::*;
#[allow(unused_imports)]
pub(super) use accounting_opps::*;
#[allow(unused_imports)]
pub(super) use security_media::*;
#[allow(unused_imports)]
pub(super) use workspaces::*;
#[allow(unused_imports)]
pub(super) use tech_documents::*;


pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/entitlements", get(entitlements))
        .route("/api/portal/rust-ui/cockpit", get(cockpit).post(cockpit_act))
        .route("/api/portal/rust-ui/catch-up", get(catch_up).post(catch_up_act))
        .route("/api/portal/rust-ui/publishing", get(publishing))
        .route("/api/portal/rust-ui/tech", get(tech).post(tech_act))
        .route("/api/portal/rust-ui/clients", get(clients))
        .route("/api/portal/rust-ui/page", get(page))
        .route("/api/portal/rust-ui/cabinet", get(cabinet))
        .route("/api/portal/rust-ui/deals", get(deals).post(deals_write))
        .route("/api/portal/rust-ui/forms", get(forms).post(forms_write))
        .route("/api/portal/rust-ui/forms/preview", axum::routing::post(forms_preview))
        .route("/api/portal/rust-ui/forms/grok", axum::routing::post(forms_grok))
        .route("/api/portal/rust-ui/projects", get(projects).post(projects_act))
        .route(
            "/api/portal/rust-ui/projects/calendar",
            get(projects_calendar).post(projects_calendar_update),
        )
        .route(
            "/api/portal/rust-ui/projects/calendar-command",
            get(projects_calendar_command),
        )
        .route("/api/portal/rust-ui/accounting", axum::routing::post(accounting_act))
        .route("/api/portal/rust-ui/opps", get(opps).post(opps_act))
        .route("/api/portal/rust-ui/listing-media", get(listing_media))
        .route("/api/portal/rust-ui/rows", get(rows))
        .route("/api/portal/rust-ui/security-users", axum::routing::put(security_users_put))
        .route("/api/portal/rust-ui/role-entitlements", axum::routing::put(role_entitlements_put))
        .route("/api/property-media/hero", axum::routing::post(property_media_hero))
        .route("/api/property-media/remove", axum::routing::post(property_media_remove))
        .route("/api/portal/property/merge-parcel", axum::routing::post(property_merge_parcel))
        .route("/api/portal/documents/{id}/file", get(portal_document_file))
        .route("/api/portal/projects/documents/{id}/signed-copy-to-come", axum::routing::post(project_document_signed_copy_to_come))
        .route(
            "/api/portal/projects/documents/{id}/signed",
            axum::routing::post(project_document_signed).layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024)),
        )
        .route("/api/portal/property-video/upload", axum::routing::post(property_video_upload))
        .route("/api/portal/property-video/finalize", axum::routing::post(property_video_finalize))
        .route(
            "/api/property-media/chunked",
            axum::routing::post(property_media_chunked).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
}
