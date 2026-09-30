//! Structural snapshot helpers.
//!
//! A response shape is a contract, and a contract test should assert it whole rather than field by field. But a raw
//! `serde_json::to_string` is not stable: object key order can depend on how the map was built, and a timestamp or a
//! generated id changes on every run. [`Snapshot`] fixes both — it canonicalises object keys to a total order, and
//! [`Snapshot::redacted`] replaces volatile leaves with a placeholder — so a snapshot is compared on structure, which
//! is what the contract is about.
//!
//! The helpers work on `serde_json::Value`, so any `Serialize` type can be snapshotted and the harness does not need
//! to know its shape.

use serde::Serialize;
use serde_json::{Map, Value};

/// Recursively sort object keys so two structurally-equal values render identically.
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut ordered = Map::new();
            for (key, child) in entries {
                ordered.insert(key.clone(), canonicalize(child));
            }
            Value::Object(ordered)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// Replace the value at every matching key, at any depth, with a placeholder.
///
/// Used for the parts of a payload that cannot be stable across runs: a generated id, a timestamp, a correlation id.
pub fn redact(value: &Value, keys: &[&str]) -> Value {
    redact_with(value, keys, "<redacted>")
}

/// Like [`redact`], with an explicit replacement.
pub fn redact_with(value: &Value, keys: &[&str], replacement: &str) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, child) in map {
                if keys.contains(&key.as_str()) {
                    out.insert(key.clone(), Value::String(replacement.to_owned()));
                } else {
                    out.insert(key.clone(), redact_with(child, keys, replacement));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| redact_with(item, keys, replacement))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A canonicalised JSON value compared on structure.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    value: Value,
}

impl Snapshot {
    /// Snapshot any serializable value.
    pub fn of<T: Serialize>(value: &T) -> Self {
        let raw = serde_json::to_value(value).expect("a snapshot subject must serialize");
        Self {
            value: canonicalize(&raw),
        }
    }

    /// Snapshot an already-built JSON value.
    pub fn of_value(value: &Value) -> Self {
        Self {
            value: canonicalize(value),
        }
    }

    /// Replace volatile leaves, at any depth, then re-canonicalise.
    pub fn redacted(self, keys: &[&str]) -> Self {
        Self {
            value: canonicalize(&redact(&self.value, keys)),
        }
    }

    /// The canonical value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The canonical value as pretty text, with a trailing newline.
    pub fn text(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.value).expect("a snapshot must render");
        text.push('\n');
        text
    }

    /// The canonical value as compact text.
    pub fn compact(&self) -> String {
        serde_json::to_string(&self.value).expect("a snapshot must render")
    }

    /// Assert that `value` has the same structure as this snapshot.
    pub fn assert_matches(&self, value: &Value) {
        let actual = canonicalize(value);
        assert_eq!(
            actual,
            self.value,
            "the response shape drifted from the snapshot\n--- expected ---\n{}\n--- actual ---\n{}\n",
            serde_json::to_string_pretty(&self.value).unwrap_or_default(),
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
    }
}

/// Assert that `value` matches a snapshot rendered as text (which is itself canonicalised first).
pub fn assert_snapshot(value: &Value, expected: &str) {
    let expected: Value =
        serde_json::from_str(expected).expect("the expected snapshot must be valid JSON");
    Snapshot::of_value(&expected).assert_matches(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_the_snapshot() {
        let left = json!({ "b": 2, "a": 1 });
        let right = json!({ "a": 1, "b": 2 });
        assert_eq!(Snapshot::of_value(&left), Snapshot::of_value(&right));
    }

    #[test]
    fn order_inside_an_array_is_part_of_the_structure() {
        let left = Snapshot::of_value(&json!([1, 2]));
        let right = Snapshot::of_value(&json!([2, 1]));
        assert_ne!(left, right);
    }

    #[test]
    fn volatile_leaves_can_be_redacted_at_any_depth() {
        let value = json!({
            "id": "abc",
            "nested": { "createdAt": "2026-09-29", "value": 1 },
            "items": [ { "id": "def" } ],
        });
        let redacted = redact(&value, &["id", "createdAt"]);
        assert_eq!(redacted["id"], "<redacted>");
        assert_eq!(redacted["nested"]["createdAt"], "<redacted>");
        assert_eq!(redacted["nested"]["value"], 1);
        assert_eq!(redacted["items"][0]["id"], "<redacted>");

        let a = Snapshot::of_value(&value).redacted(&["id", "createdAt"]);
        let b = Snapshot::of_value(&json!({
            "id": "different",
            "nested": { "createdAt": "2025-01-01", "value": 1 },
            "items": [ { "id": "also-different" } ],
        }))
        .redacted(&["id", "createdAt"]);
        assert_eq!(a, b);
    }

    #[test]
    fn a_snapshot_is_a_readable_document() {
        let snapshot = Snapshot::of(&json!({ "z": 1, "a": { "y": 2, "x": 3 } }));
        assert_eq!(
            snapshot.text(),
            "{\n  \"a\": {\n    \"x\": 3,\n    \"y\": 2\n  },\n  \"z\": 1\n}\n"
        );
    }
}
