//! GROK FILLS A FORM. The agent says what happened on the deal; Grok answers which fields that sets, and optionally
//! new document prose. It only SUGGESTS: the page applies the suggestion to the editor, and the agent still Saves and
//! Sends. Nothing here writes to the database.
//!
//! The rules Grok is given are the ones the form helper always had: only fields it is confident about, dates as
//! YYYY-MM-DD, select fields from their options, never an email, clause or price the agent did not state.

use serde::Deserialize;
use serde_json::{json, Map, Value};

const MODEL: &str = "grok-4.6";

const INSTRUCTIONS: &str = r#"You fill CulebraLuxe real-estate forms. Return ONLY JSON:
{"fieldValues":{"fieldName":"value"},"body":"optional document prose","note":"one short sentence for the agent"}
Rules:
- Only include fields you are confident about.
- Dates must be YYYY-MM-DD.
- Select fields must use one of the given options.
- Do not invent emails, legal clauses, or prices the user did not state.
- Keep existing values unless the user is changing them.
- body is optional; only replace the document body if the user described terms or narrative."#;

/// One field of the open form, as the page describes it to Grok.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokField {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub options: Vec<String>,
}

/// What Grok suggests.
#[derive(Debug, Clone, PartialEq)]
pub struct GrokFill {
    pub field_values: Map<String, Value>,
    pub body: Option<String>,
    pub note: String,
}

/// Why Grok could not help, in words for the agent.
#[derive(Debug)]
pub struct GrokFailure(pub String);

/// Asks Grok. `XAI_API_KEY` is the credential.
pub async fn fill(
    form_name: &str,
    fields: &[GrokField],
    current: &Map<String, Value>,
    document_body: &str,
    agent_said: &str,
) -> Result<GrokFill, GrokFailure> {
    let key = std::env::var("XAI_API_KEY")
        .ok()
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty());
    let Some(key) = key else {
        return Err(GrokFailure(
            "Grok is not configured on this server yet.".into(),
        ));
    };
    let described: Vec<Value> = fields
        .iter()
        .map(|field| {
            json!({
                "name": field.name,
                "label": field.label,
                "type": field.field_type,
                "options": field.options,
                "current": current.get(&field.name).and_then(Value::as_str).unwrap_or(""),
            })
        })
        .collect();
    let user = json!({ "form": form_name, "fields": described, "documentBody": document_body, "agentSaid": agent_said });
    let request = json!({
        "model": MODEL,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": INSTRUCTIONS },
            { "role": "user", "content": user.to_string() },
        ],
    });
    let unavailable =
        || GrokFailure("Grok could not fill the form right now. Try again in a moment.".into());
    let response = reqwest::Client::new()
        .post("https://api.x.ai/v1/chat/completions")
        .bearer_auth(key)
        .json(&request)
        .timeout(std::time::Duration::from_secs(45))
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    let payload: Value = response.json().await.map_err(|_| unavailable())?;
    let content = payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| GrokFailure("Grok did not return form data.".into()))?;
    parse(content, fields)
}

/// Grok's answer, read defensively: the JSON may come fenced or wrapped in prose; only non-empty text values for
/// fields the form has are kept, and a select field only takes one of its options (matched without regard to case).
pub fn parse(raw: &str, fields: &[GrokField]) -> Result<GrokFill, GrokFailure> {
    let missing = || GrokFailure("Grok did not return form data.".into());
    let text = raw.trim();
    let text = match (text.find("```"), text.rfind("```")) {
        (Some(open), Some(close)) if close > open => {
            let inner = &text[open + 3..close];
            inner.strip_prefix("json").unwrap_or(inner).trim()
        }
        _ => text,
    };
    let (start, end) = (
        text.find('{').ok_or_else(missing)?,
        text.rfind('}').ok_or_else(missing)?,
    );
    if end <= start {
        return Err(missing());
    }
    let parsed: Value = serde_json::from_str(&text[start..=end]).map_err(|_| missing())?;

    let suggested = parsed.get("fieldValues").and_then(Value::as_object);
    let mut field_values = Map::new();
    for field in fields {
        let Some(value) = suggested
            .and_then(|values| values.get(&field.name))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if field.field_type == "select" {
            if let Some(option) = field
                .options
                .iter()
                .find(|option| option.eq_ignore_ascii_case(value))
            {
                field_values.insert(field.name.clone(), json!(option));
            }
            continue;
        }
        field_values.insert(field.name.clone(), json!(value));
    }
    let text_at = |key: &str| {
        parsed
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
    };
    Ok(GrokFill {
        field_values,
        body: text_at("body").map(str::to_owned),
        note: text_at("note")
            .map(|note| note.trim().to_owned())
            .unwrap_or_else(|| "Filled from what you told Grok.".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<GrokField> {
        vec![
            GrokField {
                name: "buyerName".into(),
                label: "Buyer".into(),
                field_type: "text".into(),
                options: vec![],
            },
            GrokField {
                name: "financing".into(),
                label: "Financing".into(),
                field_type: "select".into(),
                options: vec!["Cash".into(), "Mortgage".into()],
            },
        ]
    }

    #[test]
    fn a_fenced_answer_is_read_and_select_fields_take_only_their_options() {
        let raw = "Here you go:\n```json\n{\"fieldValues\":{\"buyerName\":\" Ana Ruiz \",\"financing\":\"cash\",\"unknown\":\"x\"},\"note\":\"Set the buyer.\"}\n```";
        let fill = parse(raw, &fields()).expect("parses");
        assert_eq!(fill.field_values.get("buyerName"), Some(&json!("Ana Ruiz")));
        assert_eq!(fill.field_values.get("financing"), Some(&json!("Cash")));
        assert!(!fill.field_values.contains_key("unknown"));
        assert_eq!(fill.note, "Set the buyer.");
        assert_eq!(fill.body, None);
    }

    #[test]
    fn a_select_value_outside_its_options_is_dropped() {
        let fill =
            parse(r#"{"fieldValues":{"financing":"Seller carry"}}"#, &fields()).expect("parses");
        assert!(fill.field_values.is_empty());
        assert_eq!(fill.note, "Filled from what you told Grok.");
    }

    #[test]
    fn an_answer_without_json_is_refused() {
        assert!(parse("I could not help with that.", &fields()).is_err());
    }
}
