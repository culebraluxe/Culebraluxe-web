//! The Flight Recorder read for the TECH console: one process instance's whole trace, portal-authenticated.

#[allow(unused_imports)]
use super::*;

/// `/api/portal/flight-recorder/{id}` — the trace the console renders.
///
/// Same authorized, audited read as the internal `/v1/flight-recorder/{id}` (`execute_registered`), reached
/// through the portal's own context resolution so the Yew page can call it with the session cookie. A bad id
/// is a bad request; a missing instance is a 404.
pub(super) async fn flight_recorder(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "FLIGHT_RECORDER_INSTANCE_INVALID",
                "Flight Recorder requires a process-instance UUID.",
                false,
            ),
            &resolved,
        ));
    }
    let service = state.services().flight_recorder();
    let context = resolved.service.clone();
    let work_id = id.clone();
    let value = execute_registered(
        &state,
        "flight-recorder",
        "flight-recorder.transaction",
        json!({ "id": id }),
        async move { service.transaction(&work_id, &context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?
    .ok_or_else(|| {
        correlate(
            ApiError::not_found(
                "FLIGHT_RECORDER_NOT_FOUND",
                format!("Workflow instance not found: {id}"),
            ),
            &resolved,
        )
    })?;
    Ok(Json(to_json(value)))
}
