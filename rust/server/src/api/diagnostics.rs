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
        "target": state.db().target().as_str(),
    });

    Ok(success_with_correlation(value, &service.correlation_id))
}
