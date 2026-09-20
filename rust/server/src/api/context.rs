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
        }),
    };

    Ok(ResolvedRequestContext {
        service,
        acting_user,
    })
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
}
