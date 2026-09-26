//! THE PORTAL'S OWN ADDRESSES — what the Yew portal calls (`rust/ui/src/app/api.rs`), answered here at the same paths
//! and in the same shapes the Next relays used, so the UI does not change. Each handler resolves the portal user
//! (`ui_auth`) and asks the services; nothing here decides policy.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::context::ResolvedRequestContext;
use super::ui_auth::resolve_portal_context;
use super::v1_call::{enc, V1};
use super::{ApiError, ApiState};
use crate::service_support::CoreServiceError;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/entitlements", get(entitlements))
        .route(
            "/api/portal/rust-ui/cockpit",
            get(cockpit).post(cockpit_act),
        )
        .route("/api/portal/rust-ui/clients", get(clients))
}

/// A service failure, tagged with the request's correlation id so the incident and the answer name the same request.
fn failed(resolved: &ResolvedRequestContext) -> impl Fn(CoreServiceError) -> ApiError + '_ {
    move |error| ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortalEntitlements {
    account_type: String,
    security_level: String,
    is_root: bool,
    entitlement_codes: Vec<String>,
}

/// What the signed-in user may be offered: the shell reads it once and every screen asks it.
async fn entitlements(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<PortalEntitlements>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let level = resolved
        .service
        .principal
        .as_ref()
        .map(|p| p.level.clone())
        .unwrap_or_else(|| "GUEST".into());
    let user = resolved.acting_user;
    Ok(Json(PortalEntitlements {
        account_type: user.account_type,
        security_level: level,
        is_root: user.role_codes.iter().any(|role| role == "root"),
        entitlement_codes: user.entitlement_codes,
    }))
}

async fn cockpit_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<Json<Value>, ApiError> {
    let cockpit = state
        .services()
        .cockpit()
        .snapshot(&resolved.service)
        .await
        .map_err(failed(resolved))?;
    Ok(Json(json!({ "cockpit": cockpit })))
}

/// The Cockpit: KPIs, tasks, the featured deal, the pipeline and recent interactions, as `{ cockpit }`.
async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    cockpit_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CockpitAction {
    action: Option<String>,
    task_id: Option<String>,
}

/// The Cockpit's one command, completing a task; answers the Cockpit as it now stands.
async fn cockpit_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CockpitAction>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if input.action.as_deref() != Some("completeTask") {
        return Err(ApiError::bad_request(
            "COCKPIT_ACTION_UNSUPPORTED",
            "Unsupported Cockpit action.",
        ));
    }
    let task = input
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let Some(task) = task else {
        return Err(ApiError::bad_request(
            "COCKPIT_TASK_REQUIRED",
            "taskId is required.",
        ));
    };
    state
        .services()
        .task()
        .complete(task, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    cockpit_page(&state, &resolved).await
}

/// Query parameters, as the relays read them: absent and blank are the same.
type Params = Query<HashMap<String, String>>;

fn param<'a>(params: &'a Params, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// A 0-based page index from the query (`parseInt(..) || 0`, never negative).
fn page_index(params: &Params) -> i64 {
    param(params, "page")
        .and_then(|page| page.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0)
}

fn text(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or(Value::Null)
}

/// One-line address, from the parts a property has.
fn address_label(address: &Value) -> String {
    let part = |key: &str| {
        address
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let region = [part("state_or_province"), part("postal_code")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    [
        part("address_line1"),
        part("neighborhood"),
        part("city"),
        Some(region).filter(|r| !r.is_empty()),
        part("country"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

/// A client's record: detail, communications panel, and the properties they are tied to.
async fn client_record(v1: &V1, person: &str) -> Result<Value, ApiError> {
    let id = enc(person);
    let (detail, comms, properties) = tokio::try_join!(
        v1.get::<Value>(format!("/v1/clients/{id}")),
        v1.get::<Value>(format!("/v1/comms/{id}/panel?momentLimit=20")),
        v1.get::<Value>(format!("/v1/people/{id}/properties")),
    )?;
    let properties: Vec<Value> = properties
        .get("properties")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            let property = entry.get("property").cloned().unwrap_or_default();
            json!({
                "id": text(&property, "id"),
                "displayName": text(&property, "display_name"),
                "relation": text(entry, "relation"),
                "relationStatus": text(entry, "relation_status"),
                "address": address_label(property.get("address").unwrap_or(&Value::Null)),
            })
        })
        .collect();
    Ok(json!({ "selected": detail, "comms": comms, "properties": properties }))
}

fn merge(mut base: Value, extra: Value) -> Value {
    if let (Some(base), Value::Object(extra)) = (base.as_object_mut(), extra) {
        base.extend(extra);
    }
    base
}

/// Clients: the directory (search, page, the selected client hydrated) or one client's record (`screen=client-record`).
async fn clients(
    State(state): State<ApiState>,
    headers: HeaderMap,
    params: Params,
) -> Result<Json<Value>, ApiError> {
    let v1 = V1::portal(&state, &headers).await?;
    match param(&params, "screen").unwrap_or("clients") {
        "client-record" => {
            let Some(scope) = param(&params, "scope") else {
                return Err(ApiError::bad_request(
                    "CLIENT_SCOPE_REQUIRED",
                    "client-record requires scope.",
                ));
            };
            let record = client_record(&v1, scope).await?;
            let page =
                json!({ "rows": [], "total": 0, "page": 1, "pageSize": 50, "selectedId": scope });
            Ok(Json(json!({ "clients": merge(page, record) })))
        }
        "clients" => {
            let search = param(&params, "search").unwrap_or("");
            let directory: Value = v1
                .get(&format!(
                    "/v1/clients?search={}&sort=name&page={}&pageSize=50",
                    enc(search),
                    page_index(&params) + 1
                ))
                .await?;
            let rows: Vec<Value> = directory
                .get("rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let selected = param(&params, "selected").map(str::to_owned).or_else(|| {
                rows.first()
                    .and_then(|row| row.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            let record = match &selected {
                Some(id) => client_record(&v1, id).await?,
                None => json!({ "selected": null, "comms": null, "properties": [] }),
            };
            let rows: Vec<Value> = rows
                .iter()
                .map(|row| {
                    let activity = row.get("relationshipActivity").cloned().unwrap_or_default();
                    json!({
                        "id": text(row, "id"),
                        "displayName": text(row, "displayName"),
                        "nameResolved": text(row, "nameResolved"),
                        "role": text(row, "role"),
                        "status": text(row, "status"),
                        "primaryEmail": text(row, "primaryEmail"),
                        "primaryPhone": text(row, "primaryPhone"),
                        "observedCount": text(&activity, "observedCommunicationCount"),
                        "twoWay": text(&activity, "twoWay"),
                    })
                })
                .collect();
            let page = json!({
                "rows": rows,
                "total": text(&directory, "total"),
                "page": text(&directory, "page"),
                "pageSize": text(&directory, "pageSize"),
                "selectedId": selected,
            });
            Ok(Json(json!({ "clients": merge(page, record) })))
        }
        other => Err(ApiError::bad_request(
            "CLIENT_SCREEN_UNSUPPORTED",
            format!("unsupported client screen '{other}'"),
        )),
    }
}
