//! forge_vendor_session — existing Neon table. Same SQL as db/forge-vendor-session.ts.

use std::process::Command;

pub fn database_url() -> Option<String> {
    ["DATABASE_URL", "DATABASE_URL_DEV"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|s| !s.trim().is_empty()))
}

pub fn psql_query(sql: &str) -> Result<String, String> { psql(sql) }
pub fn sql_literal(s: &str) -> String { format!("'{}'", s.replace('\'', "''")) }

fn psql(sql: &str) -> Result<String, String> {
    let url = database_url().ok_or_else(|| "DATABASE_URL is not set".to_string())?;
    let out = Command::new("psql")
        .args(["--dbname", &url, "-At", "-v", "ON_ERROR_STOP=1", "-c", sql])
        .output()
        .map_err(|e| format!("psql: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn read_vendor_session_id(story_id: &str, lane: &str) -> Result<Option<String>, String> {
    let sql = format!(
        "SELECT session_id FROM forge_vendor_session WHERE story_id = {} AND worker_id = {} LIMIT 1",
        sql_literal(story_id), sql_literal(lane)
    );
    let raw = psql(&sql)?;
    if raw.is_empty() || raw == "\\N" { Ok(None) } else { Ok(Some(raw)) }
}

pub fn write_vendor_session_id(story_id: &str, lane: &str, session_id: Option<&str>) -> Result<(), String> {
    let sid = match session_id { Some(id) => sql_literal(id), None => "NULL".into() };
    let sql = format!(
        "INSERT INTO forge_vendor_session (story_id, worker_id, session_id, updated_at) VALUES ({}, {}, {sid}, now()) ON CONFLICT (story_id, worker_id) DO UPDATE SET session_id = excluded.session_id, updated_at = now()",
        sql_literal(story_id), sql_literal(lane)
    );
    let _ = psql(&sql)?;
    Ok(())
}
