//! Read-only diagnostics for the running process.
//!
//! Nothing here is a business operation. It answers "what has the pool actually been doing", which has been the
//! hardest question in this workspace to answer from the outside: a cold pool looks exactly like a slow database.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use super::context::resolve_engine_context;
use super::routes::{success_with_correlation, ApiSuccess};
use super::{ApiError, ApiState};

pub async fn db_metrics(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    // Internal key only, the same gate as an engine command: these are internals, not a public surface. No identity is
    // needed, so a background job or an operator script can read them without pretending to be a person.
    let service = resolve_engine_context(&state, &headers).await?;
    let snapshot = state.db().metrics();
    let target = state.db().target().as_str();

    let directory_count = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from mv_client_directory",
    )
    .fetch_one(state.db().pool())
    .await
    .map_err(|error| ApiError::from_db(db::DbFailure::from_sqlx("diagnostics.directory_count", &error)))?;
    let person_count = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from person where archived_at is null",
    )
    .fetch_one(state.db().pool())
    .await
    .map_err(|error| ApiError::from_db(db::DbFailure::from_sqlx("diagnostics.person_count", &error)))?;

    let declared_by = if std::env::var("VERCEL_ENV").ok().as_deref().is_some() {
        Some("VERCEL_ENV")
    } else if std::env::var("APP_ENV").ok().as_deref().is_some() {
        Some("APP_ENV")
    } else {
        None
    };
    let database_url = match target {
        "prod" => std::env::var("DATABASE_URL_PROD").ok(),
        _ => std::env::var("DATABASE_URL_DEV").ok(),
    };
    let neon_branch = database_url.as_deref().and_then(|value| {
        let host = value
            .split('@')
            .nth(1)?
            .split('/')
            .next()?
            .split(':')
            .next()?;
        Some(
            host.split("--")
                .next()
                .unwrap_or(host)
                .to_owned(),
        )
    });

    let value = serde_json::json!({
        "checkouts": snapshot.checkouts,
        "connectionsOpened": snapshot.connections_opened,
        "idleProbes": snapshot.idle_probes,
        "probesFailed": snapshot.probes_failed,
        "poolSize": snapshot.pool_size,
        "poolIdle": snapshot.pool_idle,
        // The headline: the share of checkouts served by a connection that was already open. Anything well below 1
        // means requests are paying a handshake, which against a remote database is the expensive path.
        "connectionReuseRate": (snapshot.connection_reuse_rate() * 1000.0).round() / 1000.0,
        "target": target,
        "db": {
            "target": target,
            "vercelEnv": std::env::var("VERCEL_ENV").ok(),
            "appEnv": std::env::var("APP_ENV").ok(),
            "declaredBy": declared_by,
            "undeclaredReason": serde_json::Value::Null,
            "neonBranch": neon_branch,
        },
        "read": {
            "directoryCount": directory_count,
            "personCount": person_count,
            "error": serde_json::Value::Null,
        },
    });

    Ok(success_with_correlation(value, &service.correlation_id))
}
