//! The Forms write: every form action the editor sends.

#[allow(unused_imports)]
use super::*;

pub(super) async fn forms_write(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let action = str_at(&body, "action").unwrap_or_default();
    let form_id = str_at(&body, "formId")
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match action {
        "create" => super::forms_write_actions::create_form(&state, &resolved, &body).await,
        "fillClient" => {
            super::forms_write_actions::fill_client(&state, &resolved, &body, form_id).await
        }
        "sendSignature" => {
            super::forms_write_actions::send_signature(&state, &resolved, &body, form_id).await
        }
        "save" | "issue" => {
            super::forms_write_actions::save_or_issue(&state, &resolved, &body, form_id, action)
                .await
        }
        _ => Err(correlate(
            ApiError::bad_request("FORM_ACTION_UNSUPPORTED", "Unsupported Forms action."),
            &resolved,
        )),
    }
}

/// snake_case keys to camelCase, all the way down: the project and work-item records are serialized snake_case by the
/// domain, and the Projects screen reads them camelCase (as the relay renamed them field by field).
pub(super) fn camel_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let mut camel = String::with_capacity(key.len());
                    let mut upper = false;
                    for ch in key.chars() {
                        if ch == '_' {
                            upper = true;
                        } else if upper {
                            camel.extend(ch.to_uppercase());
                            upper = false;
                        } else {
                            camel.push(ch);
                        }
                    }
                    (camel, camel_keys(value))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(camel_keys).collect()),
        other => other,
    }
}

pub(super) fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}
