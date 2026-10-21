//! Generic rows, and the security grant and role writes.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct RowsQuery {
    #[serde(default)]
    pub(super) screen: String,
}

/// The portal's generic rows: the Security settings tables (roles and authorities).
pub(super) async fn rows(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<RowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    match query.screen.as_str() {
        "settings-roles" | "settings-authorities" => {
            let roles = state
                .services()
                .security()
                .list_role_entitlements(&resolved.service)
                .await;
            Ok(Json(Value::Array(role_rows(&to_json(
                roles.map_err(failed(&resolved))?,
            )))))
        }
        other => Err(correlate(
            ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "ROWS_NOT_IN_RUST_YET",
                format!("The '{other}' rows have no Rust service yet."),
                false,
            ),
            &resolved,
        )),
    }
}

pub(super) const CANONICAL_INTERNAL_ROLES: &[&str] = &[
    "internal_guest",
    "user",
    "business_power_user",
    "owner",
    "root",
];

pub(super) fn is_uuid(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok_and(|id| (1..=5).contains(&id.get_version_num()))
}

/// ROOT sets one internal user's canonical role; answers the users as they now stand.
pub(super) async fn security_users_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(user), Some(role)) = (str_at(&body, "appUserId"), str_at(&body, "roleCode")) else {
        return Err(ApiError::bad_request(
            "ROLE_ASSIGNMENT_INVALID",
            "Invalid user or canonical role.",
        ));
    };
    if !is_uuid(user) || !CANONICAL_INTERNAL_ROLES.contains(&role) {
        return Err(ApiError::bad_request(
            "ROLE_ASSIGNMENT_INVALID",
            "Invalid user or canonical role.",
        ));
    }
    let security = state.services().security();
    security
        .set_user_primary_role(user, role, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let users = security
        .list_security_users(&resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "users": to_json(users) })))
}

/// ROOT grants or revokes one entitlement for one internal role; answers the role table as it now stands.
pub(super) async fn role_entitlements_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let role = str_at(&body, "roleCode").unwrap_or("");
    let action = str_at(&body, "action").unwrap_or("");
    let granted = body.get("granted").and_then(Value::as_bool);
    let role_ok =
        (1..=64).contains(&role.len()) && role.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let action_ok = (2..=101).contains(&action.len())
        && action.starts_with(|c: char| c.is_ascii_lowercase())
        && action.chars().all(|c| c.is_ascii_lowercase() || c == '.');
    let (true, true, Some(granted)) = (role_ok, action_ok, granted) else {
        return Err(ApiError::bad_request(
            "ROLE_GRANT_INVALID",
            "Invalid role or action.",
        ));
    };
    let security = state.services().security();
    security
        .set_role_entitlement(role, action, granted, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let roles = security
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "roles": to_json(roles) })))
}
