use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceExecutionMode {
    #[default]
    Inline,
    Queued,
    Ordered,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceExecutionPolicy {
    pub mode: ServiceExecutionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition_by: Option<String>,
}

impl ServiceExecutionPolicy {
    pub fn inline() -> Self {
        Self::default()
    }

    pub fn queued() -> Self {
        Self {
            mode: ServiceExecutionMode::Queued,
            partition_by: None,
        }
    }

    pub fn ordered(partition_by: impl Into<String>) -> Self {
        Self {
            mode: ServiceExecutionMode::Ordered,
            partition_by: Some(partition_by.into()),
        }
    }

    pub fn partition_key(&self, payload: &Value) -> Option<String> {
        let field = self.partition_by.as_deref()?;
        let value = payload.get(field)?;
        match value {
            Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordered_policy_resolves_the_declared_partition() {
        let policy = ServiceExecutionPolicy::ordered("propertyId");
        assert_eq!(
            policy.partition_key(&json!({ "propertyId": "p-123" })),
            Some("p-123".into())
        );
        assert_eq!(policy.partition_key(&json!({ "other": "p-123" })), None);
    }
}
