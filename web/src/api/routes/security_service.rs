//! Health, security (grants, roles, identity), guest codes, authorization, and the service kernel's catalog, control and dispatch.

#[allow(unused_imports)]
use super::*;

pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "culebraluxe-rust",
    })
}

pub(super) async fn ready(State(state): State<ApiState>) -> Result<Json<ReadyResponse>, ApiError> {
    state.db().ping().await.map_err(ApiError::from_db)?;
    Ok(Json(ReadyResponse {
        ok: true,
        database_target: state.db().target().as_str().to_owned(),
    }))
}

pub(super) async fn role_entitlements(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::security::RoleEntitlements>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .security()
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn set_role_entitlement(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<SetRoleEntitlementBody>,
) -> Result<Json<ApiSuccess<Vec<model::security::RoleEntitlements>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let security = state.services().security();
    security
        .set_role_entitlement(
            &body.role_code,
            &body.action,
            body.granted,
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    let value = security
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn security_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::security::SecurityUserRoles>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .security()
        .list_security_users(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn set_user_primary_role(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<SetUserPrimaryRoleBody>,
) -> Result<Json<ApiSuccess<Vec<model::security::SecurityUserRoles>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let security = state.services().security();
    security
        .set_user_primary_role(&body.app_user_id, &body.role_code, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    let value = security
        .list_security_users(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// One identity as the login seam consumes it: the mapped actor, or why there is none.
///
/// `kind` is the whole point of the endpoint. "unmapped" and "inactive" are different answers and the login page
/// says different things about them; collapsing them into a 401 would throw away the distinction the resolver
/// already computes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IdentityResolutionResponse {
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) acting_user: Option<IdentityActingUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) security_level: Option<String>,
}

/// The actor fields the application needs, named as the TypeScript boundary names them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IdentityActingUser {
    pub(super) app_user_id: String,
    pub(super) display_name: String,
    pub(super) email: Option<String>,
    pub(super) account_type: String,
    pub(super) role_codes: Vec<String>,
    pub(super) authority_codes: Vec<String>,
    pub(super) entitlement_codes: Vec<String>,
    pub(super) person_id: Option<String>,
}

impl From<model::ActingUser> for IdentityActingUser {
    fn from(actor: model::ActingUser) -> Self {
        Self {
            app_user_id: actor.app_user_id,
            display_name: actor.display_name,
            email: actor.email,
            account_type: actor.account_type,
            role_codes: actor.role_codes,
            authority_codes: actor.authority_codes,
            entitlement_codes: actor.entitlement_codes,
            person_id: actor.person_id,
        }
    }
}

/// Resolve an ASSERTED provider identity — the login seam's question, not "who am I".
///
/// Auth.js proves the Google subject; this decides the application mapping, in the one place that owns it. The
/// caller asserts a subject and holds no principal (see `asserted_identity_context`), which is why an unmapped
/// or inactive answer is a normal 200 response rather than an error: nothing went wrong, the answer is "no".
pub(super) async fn security_identity(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<IdentityResolutionResponse>>, ApiError> {
    let (provider, provider_subject, context) = asserted_identity_context(&state, &headers)?;
    let resolution = state
        .services()
        .security()
        .resolve_identity(&provider, &provider_subject, &context)
        .await
        .map_err(ApiError::from)?;

    let value = identity_response(resolution);

    // A system context has no acting user, so the envelope carries the correlation id rather than a resolved
    // request context: inventing an actor here would be worse than admitting there is not one.
    Ok(success_with_correlation(value, &context.correlation_id))
}

pub(super) fn identity_response(
    resolution: model::SecurityIdentityResolution,
) -> IdentityResolutionResponse {
    match resolution {
        model::SecurityIdentityResolution::Known(principal) => IdentityResolutionResponse {
            kind: "known",
            acting_user: Some(principal.acting_user.into()),
            security_level: Some(principal.level.as_str().to_owned()),
        },
        model::SecurityIdentityResolution::Unmapped => IdentityResolutionResponse {
            kind: "unmapped",
            acting_user: None,
            security_level: None,
        },
        model::SecurityIdentityResolution::Inactive => IdentityResolutionResponse {
            kind: "inactive",
            acting_user: None,
            security_level: None,
        },
    }
}

/// Email a guest a sign-in code. The public website's door: its server holds the internal key.
pub(super) async fn request_guest_code(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestCodeRequestBody>,
) -> Result<Json<ApiSuccess<GuestCodeSent>>, ApiError> {
    // The edge holds the internal key, so this is not an open door — it is a ceiling for a looping or
    // compromised edge. The limit that matters for a guesser is in the database (`security/guest.rs`).
    crate::api::rate_limit::guard_guest_code(&headers)?;
    let context = resolve_public_guest_context(&state, &headers)?;
    state
        .services()
        .guest_sign_in()
        .request_code(&body.email, body.client_ip.as_deref(), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(
        GuestCodeSent { sent: true },
        &context.correlation_id,
    ))
}

/// Check a guest's code. On success the guest exists, and the answer is the email to sign in as (the `email-code`
/// identity's subject).
pub(super) async fn verify_guest_code(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestCodeVerifyBody>,
) -> Result<Json<ApiSuccess<GuestCodeVerified>>, ApiError> {
    // The verify route is where a six-digit code is guessed, so it gets the same ceiling as the request route.
    crate::api::rate_limit::guard_guest_code(&headers)?;
    let context = resolve_public_guest_context(&state, &headers)?;
    let email = state
        .services()
        .guest_sign_in()
        .verify_code(&body.email, &body.code, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(
        GuestCodeVerified { email },
        &context.correlation_id,
    ))
}

/// Provision the external guest behind an identity the edge has proved (Google, email code), then resolve it exactly
/// as `/v1/security/identity` does. A staff identity that already maps is simply resolved; provisioning only ever
/// creates EXTERNAL guests.
pub(super) async fn provision_guest(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestProvisionBody>,
) -> Result<Json<ApiSuccess<IdentityResolutionResponse>>, ApiError> {
    let (provider, provider_subject, context) = asserted_identity_context(&state, &headers)?;
    let security = state.services().security();
    let resolution = match security
        .resolve_identity(&provider, &provider_subject, &context)
        .await
        .map_err(ApiError::from)?
    {
        model::SecurityIdentityResolution::Unmapped => {
            state
                .services()
                .guest_sign_in()
                .provision(
                    model::security::GuestClaim {
                        provider: provider.clone(),
                        subject: provider_subject.clone(),
                        email: body.email,
                        email_verified: body.email_verified,
                        display_name: body.display_name,
                    },
                    &context,
                )
                .await
                .map_err(ApiError::from)?;
            security
                .resolve_identity(&provider, &provider_subject, &context)
                .await
                .map_err(ApiError::from)?
        }
        known_or_inactive => known_or_inactive,
    };
    Ok(success_with_correlation(
        identity_response(resolution),
        &context.correlation_id,
    ))
}

/// The decision itself, shared by the identified and the anonymous doors.
///
/// The two doors differ ONLY in how the caller is resolved (a signed-in principal, or the public website), so the
/// question and its validation live here once. A second copy for the public path is how the public path would end up
/// accepting an action the catalog does not define.
pub(super) async fn authorize_decision(
    state: &ApiState,
    body: &AuthorizeBody,
    context: &ServiceContext,
) -> Result<AuthorizeResponse, ApiError> {
    let Some((action, catalog_kind)) = crate::security::catalog_action(&body.action) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "SECURITY_ACTION_UNKNOWN",
            format!("The action catalog does not contain {}.", body.action),
            false,
        ));
    };

    // Rust's catalog owns the operation kind. The caller names only the
    // action; it cannot reinterpret a command as a query (or vice versa).
    let kind = match catalog_kind {
        "query" => OperationKind::Query,
        "command" => OperationKind::Command,
        other => {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SECURITY_CATALOG_KIND_INVALID",
                format!("The action catalog contains an invalid kind for {action}: {other}."),
                false,
            ));
        }
    };

    let decision = state
        .services()
        .security()
        .decide(action, kind, context)
        .await
        .map_err(|error| ApiError::from(error))?;

    Ok(AuthorizeResponse {
        allowed: decision.allowed,
        reason: decision.reason,
        policy_id: decision.policy_id,
        mode: decision.mode,
    })
}

/// THE ONE DECISION SURFACE.
///
/// The TypeScript kernel asks here instead of carrying its own copy of the rule, so "may this principal do this?"
/// has a single answer — including the ROOT-only rule for `security.entitlement.manage` and `security.role.manage`,
/// which the TypeScript copy did not carry and therefore disagreed with this one by construction.
///
/// THE CALLER NAMES ONLY AN ACTION, and it must be one the catalog knows. Domain and operation are derived from it
/// inside the service (see `SecurityService::decide`), because the policy keys rules on those fields and a caller
/// that could rename them could dodge the `contract.execute` level floor. A command asked about as a query is
/// refused rather than answered: the kind is part of the question.
pub(super) async fn authorize_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AuthorizeBody>,
) -> Result<Json<ApiSuccess<AuthorizeResponse>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let decision = authorize_decision(&state, &body, &resolved.service)
        .await
        .map_err(|error| correlate(error, &resolved))?;

    Ok(success(decision, &resolved))
}

/// The same decision, asked by the ANONYMOUS PUBLIC SITE.
///
/// The public site reads published listings for visitors who have no session, so it has no principal to decide for —
/// and it cannot use the identified door, which requires one. This door resolves the caller to the `public-website`
/// system actor instead (`resolve_public_guest_context`, the same actor the vault's public document route uses) and
/// refuses identity headers outright: a caller either IS the public website or IS somebody, never an ambiguous
/// mixture that could be read either way by whichever rule matched first.
///
/// WHAT IT CAN REACH IS THE POLICY'S BUSINESS, NOT THIS HANDLER'S: the public actor is allowed the operations named
/// in the Rust `PUBLIC_READ_ACTIONS` list, and queries only. This door only says who is asking.
pub(super) async fn authorize_public_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AuthorizeBody>,
) -> Result<Json<ApiSuccess<AuthorizeResponse>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let correlation_id = context.correlation_id.clone();
    let decision = authorize_decision(&state, &body, &context)
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;

    // No acting user to hand to `success`, so the correlation id is passed explicitly rather than invented.
    Ok(success_with_correlation(decision, &correlation_id))
}

/// One action to decide. Rust derives its operation kind from the catalog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AuthorizeBody {
    pub(super) action: String,
    // Compatibility input only. Older edges may still send it during a rolling
    // deploy, but it is ignored; the Rust action catalog is authoritative.
    #[serde(default, rename = "kind")]
    pub(super) _legacy_kind: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AuthorizeResponse {
    pub(super) allowed: bool,
    pub(super) reason: String,
    pub(super) policy_id: String,
    pub(super) mode: &'static str,
}

/// THE ONE DECISION SURFACE.
///

pub(super) async fn service_catalog(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<services::ServiceDescriptor>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_gateway().descriptors();
    Ok(success(value, &resolved))
}

pub(super) async fn service_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<std::collections::BTreeMap<String, services::ServiceHealth>>>, ApiError>
{
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_kernel().registry().health();
    Ok(success(value, &resolved))
}

pub(super) async fn service_kernel_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::ServiceKernelHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_harness().health();
    Ok(success(value, &resolved))
}

pub(super) async fn service_runtime_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::ServiceHarnessHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    Ok(success(state.service_harness().runtime_health(), &resolved))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ServiceControlBody {
    pub(super) command: ServiceControlCommand,
}

pub(super) async fn service_control(
    State(state): State<ApiState>,
    Path(domain): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ServiceControlBody>,
) -> Result<Json<ApiSuccess<ServiceControlResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let authorization = authorize_decision(
        &state,
        &AuthorizeBody {
            action: "tech.operate".into(),
            _legacy_kind: None,
        },
        &resolved.service,
    )
    .await
    .map_err(|error| correlate(error, &resolved))?;
    if !authorization.allowed {
        return Err(ApiError::forbidden(
            "SERVICE_CONTROL_FORBIDDEN",
            "Service lifecycle control requires owner/root operational authority.",
        )
        .with_correlation(resolved.service.correlation_id.clone()));
    }

    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_harness()
        .control(&domain, body.command)
        .await
        .map_err(|error| service_dispatch_error(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

pub(super) async fn command_dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CommandRequest>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_harness()
        .execute_command(&request, &resolved.service)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    Ok(success(value, &resolved))
}

pub(super) async fn service_dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(envelope): Json<ServiceEnvelope>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_gateway()
        .dispatch(&envelope, &resolved.service)
        .await
        .map_err(|error| service_dispatch_error(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

pub(super) fn service_dispatch_error(error: ServiceDispatchError) -> ApiError {
    match error {
        ServiceDispatchError::ServiceNotFound(domain) => ApiError::not_found(
            "SERVICE_NOT_FOUND",
            format!("Service not registered for domain: {domain}"),
        ),
        ServiceDispatchError::UnknownOperation { domain, operation } => ApiError::not_found(
            "UNKNOWN_OPERATION",
            format!("Unknown service operation: {domain}.{operation}"),
        ),
        ServiceDispatchError::InvalidPayload {
            domain,
            operation,
            message,
        } => ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_SERVICE_PAYLOAD",
            format!("{domain}.{operation}: {message}"),
            false,
        ),
        ServiceDispatchError::Operation {
            code,
            message,
            retryable,
            ..
        } => {
            let status = match code.as_str() {
                "FORBIDDEN" => StatusCode::FORBIDDEN,
                "AUTHORIZATION_UNAVAILABLE"
                | "AUDIT_UNAVAILABLE"
                | "DOMAIN_EVENT_UNAVAILABLE"
                | "SERVICE_ROUTER_UNAVAILABLE"
                | "DATABASE" => StatusCode::SERVICE_UNAVAILABLE,
                _ if code.ends_with("_NOT_FOUND") => StatusCode::NOT_FOUND,
                _ if code.contains("CONFLICT") => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            ApiError::new(status, code, message, retryable)
        }
        ServiceDispatchError::ServiceDraining(domain) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_DRAINING",
            format!("Service is draining: {domain}"),
            true,
        ),
        ServiceDispatchError::ServiceStopped(domain) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_STOPPED",
            format!("Service is stopped: {domain}"),
            true,
        ),
        ServiceDispatchError::OperationPanicked {
            domain,
            operation,
            message,
        } => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SERVICE_OPERATION_PANICKED",
            format!("{domain}.{operation}: {message}"),
            true,
        ),
    }
}
