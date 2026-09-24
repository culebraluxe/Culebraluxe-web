use axum::http::HeaderMap;
use domain::{ActingUser, SecurityIdentityResolution};
use service::{ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};
use uuid::Uuid;

use super::{ApiError, ApiState};

const HEADER_INTERNAL_KEY: &str = "x-culebra-internal-key";
const HEADER_PROVIDER: &str = "x-culebra-auth-provider";
const HEADER_PROVIDER_SUBJECT: &str = "x-culebra-auth-sub";
const HEADER_CORRELATION_ID: &str = "x-culebra-correlation-id";
const HEADER_CAUSATION_ID: &str = "x-culebra-causation-id";

#[derive(Debug, Clone)]
pub struct ResolvedRequestContext {
    pub service: ServiceContext,
    pub acting_user: ActingUser,
}

pub async fn resolve_request_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<ResolvedRequestContext, ApiError> {
    validate_internal_key(state, headers)?;

    let correlation_id = header(headers, HEADER_CORRELATION_ID)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let provider = required_identity_header(headers, HEADER_PROVIDER, &correlation_id)?;
    let provider_subject =
        required_identity_header(headers, HEADER_PROVIDER_SUBJECT, &correlation_id)?;
    let causation_id = header(headers, HEADER_CAUSATION_ID)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);

    let bootstrap = ServiceContext {
        actor: ServiceActor {
            id: Some("authjs-edge".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.clone(),
        causation_id: causation_id.clone(),
        principal: None,
    };

    let mut security = state.services().security();
    let resolution = security
        .resolve_identity(provider, provider_subject, &bootstrap)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;

    let principal = match resolution {
        SecurityIdentityResolution::Known(principal) => principal,
        SecurityIdentityResolution::Unmapped => {
            return Err(ApiError::forbidden(
                "AUTH_IDENTITY_UNMAPPED",
                "Authenticated provider identity is not mapped to an application user.",
            )
            .with_correlation(correlation_id));
        }
        SecurityIdentityResolution::Inactive => {
            return Err(ApiError::forbidden(
                "AUTH_IDENTITY_INACTIVE",
                "Authenticated provider identity does not resolve to an active application user.",
            )
            .with_correlation(correlation_id));
        }
    };

    let acting_user = principal.acting_user;
    let service = ServiceContext {
        actor: ServiceActor {
            id: Some(acting_user.app_user_id.clone()),
            kind: ServiceActorKind::User,
        },
        correlation_id,
        causation_id,
        principal: Some(ServicePrincipal {
            app_user_id: acting_user.app_user_id.clone(),
            level: principal.level.as_str().into(),
            role_codes: acting_user.role_codes.clone(),
            account_type: acting_user.account_type.clone(),
            entitlement_codes: acting_user.entitlement_codes.clone(),
        }),
    };

    Ok(ResolvedRequestContext {
        service,
        acting_user,
    })
}

/// The anonymous website is represented as GUEST in the service layer. Only
/// the dedicated public Vault route uses this context; ordinary Vault reads
/// continue to require a resolved application user.
pub fn resolve_public_guest_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<ServiceContext, ApiError> {
    validate_internal_key(state, headers)?;
    if identity_header_presence(headers) != (false, false) {
        return Err(ApiError::unauthorized(
            "GUEST_IDENTITY_UNEXPECTED",
            "Public document reads must not carry identity headers.",
        ));
    }
    Ok(ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: header(headers, HEADER_CORRELATION_ID)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        causation_id: None,
        principal: None,
    })
}

/// A context for engine commands, which may arrive with a user or without one.
///
/// An interactive command comes from a page and gets attributed to the person who ran it. A background command does not:
/// the recovery pass and the agreements MQ consumer have no browser session, and the engine binary they used to call
/// required no identity at all. Both are legitimate, so the internal key is the gate and the user is optional.
///
/// The distinction is not a separate field, it is `service.principal`: present for a resolved person, absent for the
/// engine itself. That is how the rest of the service layer already records "a person did this" versus "the system did
/// this", so an audit of an engine command reads like an audit of anything else.
pub async fn resolve_engine_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<ServiceContext, ApiError> {
    validate_internal_key(state, headers)?;

    let correlation_id = header(headers, HEADER_CORRELATION_ID)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    match identity_header_presence(headers) {
        // Both present: an interactive command, resolved exactly as a read would be, so an unmapped or inactive
        // identity is still refused rather than silently downgraded to a system actor.
        (true, true) => {
            let resolved = resolve_request_context(state, headers).await?;
            return Ok(resolved.service);
        }
        // Neither present: a background command, attributed to the workflow engine itself.
        (false, false) => {}
        // One without the other is a malformed caller, not a background job.
        _ => {
            return Err(ApiError::unauthorized(
                "AUTH_IDENTITY_INCOMPLETE",
                "Engine commands must present both identity headers or neither.",
            )
            .with_correlation(correlation_id))
        }
    }

    Ok(ServiceContext {
        actor: ServiceActor {
            id: Some("workflow-engine".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id,
        causation_id: header(headers, HEADER_CAUSATION_ID)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned),
        principal: None,
    })
}

fn identity_header_presence(headers: &HeaderMap) -> (bool, bool) {
    (
        header(headers, HEADER_PROVIDER).is_some_and(|value| !value.trim().is_empty()),
        header(headers, HEADER_PROVIDER_SUBJECT).is_some_and(|value| !value.trim().is_empty()),
    )
}

/// The login seam's context: an ASSERTED provider identity, not "who am I".
///
/// `resolve_request_context` answers who is calling, and fails when the identity does not map to an active user.
/// The login seam has the opposite question — Auth.js has proved a Google subject and *nobody knows yet* whether
/// it maps, or maps to someone inactive — so this returns the asserted identity plus a context to ask with.
///
/// THE ACTOR IS THE RESERVED `authjs-edge` SYSTEM IDENTITY, and that is a policy boundary rather than a label:
/// the authorization layer admits `security.identity.resolve` for that actor and no principal, which is what
/// makes "resolve this subject" a different act from "act as this subject". A signed-in principal cannot use
/// this to enumerate or resolve other people's identities.
pub fn asserted_identity_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<(String, String, ServiceContext), ApiError> {
    validate_internal_key(state, headers)?;

    let correlation_id = header(headers, HEADER_CORRELATION_ID)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let provider = required_identity_header(headers, HEADER_PROVIDER, &correlation_id)?;
    let provider_subject = required_identity_header(headers, HEADER_PROVIDER_SUBJECT, &correlation_id)?;

    let context = ServiceContext {
        actor: ServiceActor {
            id: Some(service::AUTHJS_EDGE_ACTOR.to_owned()),
            kind: ServiceActorKind::System,
        },
        correlation_id,
        causation_id: header(headers, HEADER_CAUSATION_ID).map(str::to_owned),
        principal: None,
    };

    Ok((
        provider.to_owned(),
        provider_subject.to_owned(),
        context,
    ))
}

fn validate_internal_key(state: &ApiState, headers: &HeaderMap) -> Result<(), ApiError> {
    let supplied = header(headers, HEADER_INTERNAL_KEY);
    if supplied != Some(state.internal_api_key()) {
        return Err(ApiError::unauthorized(
            "INTERNAL_AUTH_REQUIRED",
            "Rust API request is not from the trusted application edge.",
        ));
    }
    Ok(())
}

fn required_identity_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
    correlation_id: &str,
) -> Result<&'a str, ApiError> {
    let value = header(headers, name)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ApiError::unauthorized(
                "AUTH_IDENTITY_REQUIRED",
                "Authenticated provider identity headers are required.",
            )
            .with_correlation(correlation_id.to_owned())
        })?;

    if value.len() > 512 {
        return Err(ApiError::unauthorized(
            "AUTH_IDENTITY_INVALID",
            "Authenticated provider identity header is invalid.",
        )
        .with_correlation(correlation_id.to_owned()));
    }

    Ok(value)
}

fn header<'a>(headers: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_header_rejects_blank_and_oversized_values() {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_PROVIDER, "   ".parse().unwrap());
        assert!(required_identity_header(&headers, HEADER_PROVIDER, "c1").is_err());

        let oversized = "x".repeat(513);
        headers.insert(HEADER_PROVIDER, oversized.parse().unwrap());
        assert!(required_identity_header(&headers, HEADER_PROVIDER, "c2").is_err());
    }

    #[test]
    fn engine_calls_are_either_a_user_or_a_background_job_never_half_of_each() {
        // Neither header: a background command, which the engine must keep accepting - the recovery pass and the
        // agreements consumer have no session, and the binary they replaced required no identity at all.
        let none = identity_header_presence(&HeaderMap::new());
        assert_eq!(none, (false, false));

        // Both: an interactive command, attributed to a person.
        let mut both = HeaderMap::new();
        both.insert(HEADER_PROVIDER, "authjs".parse().unwrap());
        both.insert(HEADER_PROVIDER_SUBJECT, "user-1".parse().unwrap());
        assert_eq!(identity_header_presence(&both), (true, true));

        // One without the other is not a background job, it is a broken caller.
        let mut half = HeaderMap::new();
        half.insert(HEADER_PROVIDER, "authjs".parse().unwrap());
        assert_eq!(identity_header_presence(&half), (true, false));

        // Blank counts as absent, so an empty header cannot downgrade a user command to a system actor.
        let mut blank = HeaderMap::new();
        blank.insert(HEADER_PROVIDER, "  ".parse().unwrap());
        blank.insert(HEADER_PROVIDER_SUBJECT, "user-1".parse().unwrap());
        assert_eq!(identity_header_presence(&blank), (false, true));
    }
}
