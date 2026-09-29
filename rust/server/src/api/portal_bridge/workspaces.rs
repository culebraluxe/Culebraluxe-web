//! Entitlements, Cockpit, Publishing, Catch-Up, the Support screens and Meta's phone numbers.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PortalEntitlements {
    pub(super) account_type: String,
    pub(super) security_level: String,
    pub(super) is_root: bool,
    pub(super) entitlement_codes: Vec<String>,
}

/// What the signed-in user may be offered: the shell reads it once and every screen asks it.
pub(super) async fn entitlements(
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

pub(super) async fn cockpit_page(
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

pub(super) async fn publishing(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let snapshot = state
        .services()
        .publishing()
        .snapshot(&resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "publishing": snapshot })))
}

pub(super) async fn catch_up_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<Json<Value>, ApiError> {
    let snapshot = state
        .services()
        .catch_up()
        .snapshot(&resolved.service)
        .await
        .map_err(failed(resolved))?;
    Ok(Json(json!({ "catchUp": snapshot })))
}

/// Relationship Catch-Up: one deterministic reason per person, highest priority first.
pub(super) async fn catch_up(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    catch_up_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CatchUpAction {
    pub(super) action: String,
    pub(super) person_id: String,
    pub(super) reason_code: String,
    pub(super) days: Option<i32>,
}

pub(super) async fn catch_up_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CatchUpAction>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    match input.action.as_str() {
        "handle" => {
            state
                .services()
                .catch_up()
                .handle(&input.person_id, &input.reason_code, &resolved.service)
                .await
                .map_err(failed(&resolved))?;
        }
        "snooze" => {
            state
                .services()
                .catch_up()
                .snooze(
                    &input.person_id,
                    &input.reason_code,
                    input.days.unwrap_or(3),
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;
        }
        _ => {
            return Err(ApiError::bad_request(
                "CATCH_UP_ACTION_UNSUPPORTED",
                "Catch-Up action must be handle or snooze.",
            ));
        }
    }
    catch_up_page(&state, &resolved).await
}

/// The Cockpit: KPIs, tasks, the featured deal, the pipeline and recent interactions, as `{ cockpit }`.
pub(super) async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    cockpit_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CockpitAction {
    pub(super) action: Option<String>,
    pub(super) task_id: Option<String>,
}

/// The Cockpit's one command, completing a task; answers the Cockpit as it now stands.
pub(super) async fn cockpit_act(
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

pub(super) const SUPPORT_SCREENS: &[&str] = &["system-health", "db-test", "whatsapp-meta", "security", "settings-users"];

/// One SUPPORT screen's payload. Diagnostics carry posture and counts only: never a token, secret, hash or URL.
pub(super) async fn support_payload(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    screen: &str,
    scope: Option<&str>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let support = services.support();
    Ok(match screen {
        "db-test" => {
            // Every client, a page at a time: the screen proves the database answers and shows what it holds.
            let clients = services.clients();
            let mut rows = Vec::new();
            let mut page = 1;
            let total = loop {
                let request = ClientDirectoryPageRequest {
                    search: String::new(), status: None, role: None, sort: "name".into(), page, page_size: 100,
                };
                let answer = to_json(clients.directory(&request, context).await.map_err(failed(resolved))?);
                let batch = answer.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
                let total = at(&answer, "total").as_i64().unwrap_or(0);
                let size = at(&answer, "pageSize").as_i64().unwrap_or(100).max(1);
                let done = batch.is_empty() || page * size >= total;
                rows.extend(batch.into_iter().map(|row| json!({
                    "id": at(&row, "id"), "displayName": at(&row, "displayName"), "role": at(&row, "role"),
                    "status": at(&row, "status"), "email": at(&row, "primaryEmail"), "phone": at(&row, "primaryPhone"),
                })));
                if done {
                    break total;
                }
                page += 1;
            };
            json!({ "dbTest": { "connected": true, "clientCount": total, "clients": rows } })
        }
        "settings-users" => {
            let users = services.security().list_security_users(context).await.map_err(failed(resolved))?;
            json!({ "securityUsers": to_json(users) })
        }
        "security" => {
            let security = services.security();
            let (status, grants) = tokio::join!(support.security_status(context), security.list_role_entitlements(context));
            let break_glass = break_glass_readiness(state, resolved).await?;
            json!({ "security": {
                "status": to_json(status.map_err(failed(resolved))?),
                "breakGlass": to_json(break_glass),
                "roleEntitlements": to_json(grants.map_err(failed(resolved))?),
            } })
        }
        "whatsapp-meta" => json!({ "whatsAppMeta": meta_phones().await }),
        _ => {
            let (health, diagnostics) = tokio::join!(support.system_health(context), support.workflow_diagnostics(context));
            let mut diagnostics = to_json(diagnostics.map_err(failed(resolved))?);
            let detail = match scope {
                Some(id) => to_json(support.workflow_detail(id, context).await.map_err(failed(resolved))?),
                None => Value::Null,
            };
            if let Some(object) = diagnostics.as_object_mut() {
                object.insert("detail".into(), detail);
            }
            json!({ "systemHealth": {
                "health": to_json(health.map_err(failed(resolved))?),
                "environment": environment_readiness(),
                "diagnostics": diagnostics,
            } })
        }
    })
}

/// Which settings this deployment has — booleans only, never a value.
pub(super) fn environment_readiness() -> Value {
    let set = |key: &str| std::env::var(key).is_ok_and(|value| !value.trim().is_empty());
    let value = |key: &str| std::env::var(key).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    let production = ["APP_ENV", "VERCEL_ENV"].iter().any(|key| value(key).is_some_and(|v| v.eq_ignore_ascii_case("production")));
    let (prod_db, dev_db) = (value("DATABASE_URL_PROD"), value("DATABASE_URL_DEV"));
    let database = if production { prod_db.is_some() } else { dev_db.is_some() };
    let separated = matches!((&prod_db, &dev_db), (Some(prod), Some(dev)) if prod != dev);
    let auth_secret = set("AUTH_SECRET");
    let auth_provider = set("AUTH_GOOGLE_ID") && set("AUTH_GOOGLE_SECRET");
    let maps = if production { set("GOOGLE_MAPS_API_KEY") } else { set("GOOGLE_MAPS_DEMO_KEY") || set("GOOGLE_MAPS_API_KEY") };
    let maps_demo_absent = !production || !set("GOOGLE_MAPS_DEMO_KEY");
    let mux = if production {
        set("MUX_TOKEN_ID_PROD") && set("MUX_TOKEN_SECRET_PROD")
    } else {
        set("MUX_TOKEN_ID_DEV") && set("MUX_TOKEN_SECRET_DEV")
    };
    let signature_enabled = value("BROKER_SIGNATURE_ENABLED").is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let signature = ["BROKER_SIGNATURE_APP_USER_ID", "BROKER_SIGNATURE_MEDIA_ID", "BROKER_SIGNATURE_SIGNER_NAME", "BROKER_SIGNATURE_LICENSE_NUMBER"]
        .iter()
        .all(|key| set(key));
    let all_required = if production {
        [database, auth_secret, auth_provider, maps, maps_demo_absent, mux, signature_enabled, signature].iter().all(|ok| *ok)
    } else {
        database
    };
    json!({
        "isProduction": production,
        "databaseConfigured": database,
        "databaseDevProdSeparated": separated,
        "authSecretConfigured": auth_secret,
        "authProviderConfigured": auth_provider,
        "breakGlassConfigured": set("AUTH_BREAK_GLASS_APP_USER_ID") && set("AUTH_BREAK_GLASS_SECRET_HASH"),
        "breakGlassEnabled": std::env::var("AUTH_BREAK_GLASS_ENABLED").is_ok_and(|v| v == "true"),
        "googleMapsKeyConfigured": maps,
        "googleMapsDemoKeyAbsentInProduction": maps_demo_absent,
        "muxConfigured": mux,
        "brokerSignatureConfigured": signature,
        "brokerSignatureEnabled": signature_enabled,
        "allProductionRequiredConfigured": all_required,
    })
}

/// The WhatsApp diagnostic: ONE read-only GET of this business account's phone numbers from Meta, made only when the
/// screen is opened, exactly as the TypeScript page did. It changes nothing at Meta. The token is used in a header and
/// never returned; what crosses is the account id, whether a token exists, Meta's error if any, and the phone fields.
pub(super) async fn meta_phones() -> Value {
    const DEFAULT_WABA_ID: &str = "1605543247626812";
    const GRAPH_VERSION: &str = "v23.0";
    let waba_id = std::env::var("WHATSAPP_WABA_ID").ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_WABA_ID.to_owned());
    let answer = |phones: Vec<Value>, error: Option<String>, token: bool| {
        json!({ "wabaId": waba_id, "phones": phones, "error": error, "tokenConfigured": token })
    };
    let Some(token) = std::env::var("WHATSAPP_ACCESS_TOKEN").ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) else {
        return answer(Vec::new(), Some("WHATSAPP_ACCESS_TOKEN is not configured.".into()), false);
    };
    let url = format!("https://graph.facebook.com/{GRAPH_VERSION}/{}/phone_numbers", waba_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let response = match reqwest::Client::new().get(url).bearer_auth(token).send().await {
        Ok(response) => response,
        Err(error) => return answer(Vec::new(), Some(error.to_string()), true),
    };
    let status = response.status();
    let payload: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let message = payload.pointer("/error/message").and_then(Value::as_str).map(str::to_owned)
            .unwrap_or_else(|| format!("Meta returned HTTP {}.", status.as_u16()));
        return answer(Vec::new(), Some(message), true);
    }
    let phones = payload.get("data").and_then(Value::as_array).into_iter().flatten().map(|phone| json!({
        "id": at(phone, "id"),
        "displayPhoneNumber": at(phone, "display_phone_number"),
        "verifiedName": at(phone, "verified_name"),
        "qualityRating": at(phone, "quality_rating"),
        "codeVerificationStatus": at(phone, "code_verification_status"),
    })).collect();
    answer(phones, None, true)
}
