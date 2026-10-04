//! Forge job payload encoding.
//!
//! The durable job envelope that travels from the bridge to the worker.
//! Kept separate so the bridge and the worker agree on the exact shape
//! without either owning the other's logic.

use crate::engine::job::ForgeJobRequest;
use workflow::Value;

/// Encode a ForgeJobRequest into the generic job payload Value.
/// This is the single source of truth for the payload shape.
pub fn request_payload(request: &ForgeJobRequest) -> Value {
    let mut payload = Value::object();
    payload.insert("serviceKey", Value::from(request.service_key.as_str()));
    payload.insert("nodeId", Value::from(request.node_id.as_str()));
    payload.insert("taskId", Value::from(request.task.task_id.as_str()));
    payload.insert(
        "processInstanceId",
        Value::from(request.task.process_instance_id.as_str()),
    );
    payload.insert("storyId", Value::from(request.task.story_id.as_str()));
    payload.insert("tokenId", Value::from(request.task.token_id.clone()));
    payload
}

/// Extract a required string from the payload.
pub fn required_string(payload: &Value, key: &str) -> Result<String, workflow::WorkflowError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| workflow::WorkflowError::generic(format!("Forge job payload missing {key}")))
}

/// Extract an optional string from the payload.
pub fn optional_string(payload: &Value, key: &str) -> Result<Option<String>, workflow::WorkflowError> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        _ => Err(workflow::WorkflowError::generic(format!(
            "Forge job payload has invalid {key}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::job::ForgeJobRequest;
    use crate::engine::runtime::ActiveForgeRoleTask;
    use workflow::TaskStatus;

    fn sample_task() -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: "task-123".into(),
            process_instance_id: "proc-456".into(),
            story_id: "ENG-TEST-01".into(),
            token_id: Some("token-789".into()),
            node_id: Some("smith".into()),
            status: TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
        }
    }

    #[test]
    fn payload_roundtrip_preserves_all_fields() {
        let request = ForgeJobRequest {
            service_key: "forge.smith".into(),
            node_id: "smith".into(),
            task: sample_task(),
        };

        let payload = request_payload(&request);

        assert_eq!(payload.get("serviceKey").and_then(Value::as_str), Some("forge.smith"));
        assert_eq!(payload.get("nodeId").and_then(Value::as_str), Some("smith"));
        assert_eq!(payload.get("taskId").and_then(Value::as_str), Some("task-123"));
        assert_eq!(
            payload.get("processInstanceId").and_then(Value::as_str),
            Some("proc-456")
        );
        assert_eq!(payload.get("storyId").and_then(Value::as_str), Some("ENG-TEST-01"));
        assert_eq!(
            payload.get("tokenId").and_then(Value::as_str),
            Some("token-789")
        );
    }
}