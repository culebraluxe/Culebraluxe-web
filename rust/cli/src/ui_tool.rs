//! UI operator utilities previously implemented as Node scripts.

use serde_json::{Map, Value};
use std::error::Error;
use std::fs;
use std::io;
use uuid::Uuid;

const ENDPOINTS: &[(&str, &str)] = &[
    (
        "portal-page-db-test",
        "/api/portal/rust-ui/page?screen=db-test",
    ),
    (
        "portal-page-system-health",
        "/api/portal/rust-ui/page?screen=system-health",
    ),
    (
        "portal-page-whatsapp-meta",
        "/api/portal/rust-ui/page?screen=whatsapp-meta",
    ),
    (
        "portal-page-security",
        "/api/portal/rust-ui/page?screen=security",
    ),
    (
        "portal-page-settings-users",
        "/api/portal/rust-ui/page?screen=settings-users",
    ),
    (
        "portal-page-accounting",
        "/api/portal/rust-ui/page?screen=accounting",
    ),
    (
        "portal-page-accounting-pnl",
        "/api/portal/rust-ui/page?screen=accounting-pnl&from=&to=",
    ),
    (
        "clients-list",
        "/api/portal/rust-ui/clients?screen=clients&page=0&search=",
    ),
    ("guest-session", "/api/rust-ui/guest"),
];

pub async fn dispatch(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("capture-fixtures") => capture_fixtures(&args[1..]).await,
        _ => Err(io::Error::other("usage: ui capture-fixtures [base-url]").into()),
    }
}

async fn capture_fixtures(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() > 1 {
        return Err(io::Error::other("usage: ui capture-fixtures [base-url]").into());
    }

    let base = args
        .first()
        .map(String::as_str)
        .unwrap_or("http://127.0.0.1:3000")
        .trim_end_matches('/');
    let root = crate::forge::repo_root();
    let out = root.join("rust/ui/fixtures");
    fs::create_dir_all(&out)?;

    let client = reqwest::Client::new();
    let mut failed = 0usize;

    for (name, path) in ENDPOINTS {
        let response = client.get(format!("{base}{path}")).send().await?;
        let status = response.status();
        let text = response.text().await?;
        let parsed = serde_json::from_str::<Value>(&text);

        if !status.is_success() || parsed.is_err() {
            failed += 1;
            let preview: String = text.chars().take(120).collect();
            eprintln!("FAIL {name}: {status} {preview}");
            continue;
        }

        let scrubbed = scrub(parsed.expect("checked above"));
        let mut encoded = serde_json::to_string_pretty(&scrubbed)?;
        encoded.push('\n');
        fs::write(out.join(format!("{name}.json")), encoded)?;
        println!("ok   {name} ({} bytes received, scrubbed)", text.len());
    }

    if failed == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{failed} UI fixture capture(s) failed"
        ))
        .into())
    }
}

fn scrub(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().take(2).map(scrub).collect()),
        Value::Object(values) => {
            let mapped: Map<String, Value> = values
                .into_iter()
                .map(|(key, value)| (key, scrub(value)))
                .collect();
            Value::Object(mapped)
        }
        Value::String(value) => Value::String(scrub_string(value)),
        other => other,
    }
}

fn scrub_string(value: String) -> String {
    if value.is_empty() || keep_string(&value) {
        return value;
    }
    if value.contains('@') {
        return "person@example.com".into();
    }
    if looks_like_phone(&value) {
        return "+10000000000".into();
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return "https://example.com/".into();
    }
    "Sample text".into()
}

fn keep_string(value: &str) -> bool {
    if Uuid::parse_str(value).is_ok() {
        return true;
    }

    let bytes = value.as_bytes();
    let looks_date = bytes.len() >= 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit);

    if looks_date {
        return true;
    }

    value.len() <= 32
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase()
                || ch.is_ascii_digit()
                || matches!(ch, '_' | '.' | '-')
        })
}

fn looks_like_phone(value: &str) -> bool {
    let mut digits = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
        } else if !matches!(ch, '+' | ' ' | '(' | ')' | '.' | '-') {
            return false;
        }
    }
    digits >= 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_limits_arrays_and_removes_people() {
        let input = serde_json::json!([
            {"name":"Lisa Penfield","email":"lisa@example.com","status":"active"},
            {"name":"Another Person","phone":"+1 (787) 555-1212"},
            {"name":"third"}
        ]);
        let scrubbed = scrub(input);
        let array = scrubbed.as_array().expect("array");

        assert_eq!(array.len(), 2);
        assert_eq!(array[0]["name"], "Sample text");
        assert_eq!(array[0]["email"], "person@example.com");
        assert_eq!(array[0]["status"], "active");
        assert_eq!(array[1]["phone"], "+10000000000");
    }

    #[test]
    fn ids_dates_and_codes_are_kept() {
        assert_eq!(scrub_string("active".into()), "active");
        assert_eq!(
            scrub_string("2026-09-29T12:00:00Z".into()),
            "2026-09-29T12:00:00Z"
        );
        assert_eq!(
            scrub_string("550e8400-e29b-41d4-a716-446655440000".into()),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }
}
