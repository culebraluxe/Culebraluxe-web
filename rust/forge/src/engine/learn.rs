//! Rust Forge learn loop.
//!
//! Ports the unattended learn pass that used to live in agent-runtime/learn-loop.ts.
//! It observes recent code changes + stale claims, files at most one learn story,
//! dedupes against open learn work, and never fixes/merges/promotes its own finding.

use crate::engine::vendor_session::with_shared;
use sqlx::Row;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_FILES: usize = 40;
const WINDOW_HOURS: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Severity { P0, Normal }

#[derive(Debug, Clone)]
struct Candidate {
    pattern: String,
    key: String,
    severity: Severity,
    title: String,
    evidence: Vec<String>,
    hit_count: usize,
    first_seen: String,
    last_seen: String,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn anchor_path(root: &Path) -> PathBuf {
    root.join(".forge-context").join("learn-last-run.json")
}

fn read_anchor_secs(root: &Path) -> Option<u64> {
    let raw = fs::read_to_string(anchor_path(root)).ok()?;
    let key = "\"unix\":";
    let pos = raw.find(key)? + key.len();
    raw[pos..].trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn write_anchor(root: &Path, key: Option<&str>) -> Result<(), String> {
    let path = anchor_path(root);
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let safe = key.unwrap_or("").replace('\\', "\\\\").replace('"', "\\\"");
    fs::write(path, format!("{{\"unix\":{},\"lastKey\":\"{}\"}}\n", now_secs(), safe))
        .map_err(|e| e.to_string())
}

fn code_path(path: &str) -> bool {
    let good = path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".js") ||
        path.ends_with(".mjs") || path.ends_with(".rs");
    good &&
        !path.starts_with("docs/") &&
        !path.starts_with("node_modules/") &&
        !path.starts_with(".next/") &&
        !path.starts_with(".vercel/") &&
        !path.starts_with(".forge/") &&
        !path.starts_with("testv2/") &&
        !path.contains(".test.") &&
        !path.contains(".spec.") &&
        path != "agent-runtime/silent-failure-patterns.ts"
}

fn changed_files(root: &Path, since: u64) -> Vec<(String, String)> {
    let since_arg = format!("@{since}");
    let out = Command::new("git")
        .current_dir(root)
        .args(["log", "--since", &since_arg, "--name-only", "--pretty=format:"])
        .output();
    let Ok(out) = out else { return vec![] };
    if !out.status.success() { return vec![]; }
    let mut seen = BTreeSet::new();
    let mut files = vec![];
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if p.is_empty() || !code_path(p) || !seen.insert(p.to_string()) { continue; }
        if let Ok(content) = fs::read_to_string(root.join(p)) {
            files.push((p.to_string(), content));
            if files.len() >= MAX_FILES { break; }
        }
    }
    files
}

fn line_of(content: &str, byte: usize) -> usize {
    content[..byte.min(content.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

fn push_candidate(map: &mut BTreeMap<String, Candidate>, pattern: &str, path: &str, line: usize) {
    let key = format!("{pattern}:{path}");
    let evidence = format!("{path}:{line}");
    let entry = map.entry(key.clone()).or_insert_with(|| Candidate {
        pattern: pattern.into(),
        key,
        severity: Severity::Normal,
        title: format!("{pattern} in {path}"),
        evidence: vec![],
        hit_count: 0,
        first_seen: "recent-window".into(),
        last_seen: "recent-window".into(),
    });
    entry.hit_count += 1;
    if entry.evidence.len() < 5 { entry.evidence.push(evidence); }
}

fn scan_silent_failures(files: &[(String, String)]) -> Vec<Candidate> {
    let captures = ["captureServerError", "captureServerLog", "captureError", "recordError", "withApiHandler", "withServerErrorCapture"];
    let mut map = BTreeMap::new();

    for (path, content) in files {
        let compact = content.replace("\r", "");
        for needle in ["catch {}", "catch{}", "catch (_) {}", "catch(_){ }"] {
            let mut from = 0;
            while let Some(i) = compact[from..].find(needle) {
                let at = from + i; push_candidate(&mut map, "empty-catch", path, line_of(&compact, at)); from = at + needle.len();
            }
        }
        for needle in ["=> []", "=> null", "=> undefined", "=> 0", "=> ''", "=> \"\""] {
            let mut from = 0;
            while let Some(i) = compact[from..].find(".catch(") {
                let at = from + i;
                let tail = &compact[at..compact.len().min(at + 180)];
                if tail.contains(needle) { push_candidate(&mut map, "swallowed-catch", path, line_of(&compact, at)); }
                from = at + 7;
            }
        }
        let server = path.starts_with("app/") || path.starts_with("services/") || path.contains("/app/") || path.contains("/services/");
        let captured = captures.iter().any(|m| compact.contains(m));
        if server && !captured {
            let mut from = 0;
            while let Some(i) = compact[from..].find("console.error(") {
                let at = from + i; push_candidate(&mut map, "console-error-without-capture", path, line_of(&compact, at)); from = at + 14;
            }
        }
        if compact.contains("catch") && compact.contains("status: 500") && !captured {
            if let Some(at) = compact.find("status: 500") {
                push_candidate(&mut map, "bare-500-in-catch", path, line_of(&compact, at));
            }
        }
    }
    map.into_values().collect()
}

fn stale_candidate(minutes: i64) -> Result<Option<Candidate>, String> {
    with_shared(|db, rt| rt.block_on(async move {
        let rows = sqlx::query(
            "select id::text, updated_at::text from agent_work_item
             where state in ('Claimed','Running','Paused')
               and updated_at < now() - ($1::text || ' minutes')::interval
             order by updated_at asc"
        ).bind(minutes.max(0).to_string()).fetch_all(db.pool()).await.map_err(|e| e.to_string())?;
        if rows.is_empty() { return Ok(None); }
        let mut evidence = vec![]; let mut first = String::new(); let mut last = String::new();
        for (i,row) in rows.iter().enumerate() {
            let id:String=row.try_get(0).map_err(|e|e.to_string())?;
            let at:String=row.try_get(1).unwrap_or_default();
            if i==0 { first=at.clone(); } last=at;
            if evidence.len()<5 { evidence.push(format!("agent_work_item:{id}")); }
        }
        Ok(Some(Candidate{
            pattern:"stale-claim".into(), key:"stale-claim".into(), severity:Severity::P0,
            title:format!("{} abandoned work claim(s)", rows.len()), evidence, hit_count:rows.len(),
            first_seen:first,last_seen:last,
        }))
    }))?
}

fn open_keys() -> Result<BTreeSet<String>, String> {
    with_shared(|db, rt| rt.block_on(async {
        let rows:Vec<String>=sqlx::query_scalar(
            "select learn_pattern_key from agent_work_item
             where learn_pattern_key is not null and state in ('Ready','Claimed','Running','Paused')
             union
             select learn_pattern_key from forge_batch_item
             where learn_pattern_key is not null and state='Staged'"
        ).fetch_all(db.pool()).await.map_err(|e|e.to_string())?;
        Ok(rows.into_iter().collect())
    }))?
}

fn story_id(key:&str)->String {
    let slug:String=key.chars().map(|c| if c.is_ascii_alphanumeric(){c.to_ascii_uppercase()}else{'-'}).collect();
    let slug=slug.trim_matches('-').chars().take(60).collect::<String>();
    format!("LEARN-{slug}-{}", now_secs())
}

fn instructions(c:&Candidate)->String {
    format!(
        "Filed by the Rust learn loop: pattern {} ({} hit(s)). Lead and Architect decide SMITH or HOLD; the loop does not choose the fix. Assay does not ship code. Never auto-merge, never auto-promote a decision. Evidence: {}.",
        c.key,c.hit_count,c.evidence.join(", ")
    )
}

fn ensure_staging(db:&db::Database, rt:&tokio::runtime::Runtime)->Result<String,String>{
    rt.block_on(async{
        if let Some(id)=sqlx::query_scalar::<_,String>("select id::text from forge_batch where status='Staged' order by created_at desc limit 1")
            .fetch_optional(db.pool()).await.map_err(|e|e.to_string())? { return Ok(id); }
        sqlx::query_scalar::<_,String>(
            "insert into forge_batch(label,status,note) values('staging','Staged','built by Rust learn loop') returning id::text"
        ).fetch_one(db.pool()).await.map_err(|e|e.to_string())
    })
}

fn file_candidate(c:&Candidate)->Result<String,String>{
    let id=story_id(&c.key); let notes=instructions(c);
    with_shared(|db,rt|{
        rt.block_on(async{
            sqlx::query(
                "insert into storyboard_story(id,workstream,title,priority,status,notes,goal,completion,rollup)
                 values($1,'ENGINEERING',$2,$3,'Planned',$4,$5,0,true)
                 on conflict(id) do nothing"
            ).bind(&id).bind(format!("learn: {}",c.key)).bind(if c.severity==Severity::P0{"High"}else{"Medium"})
             .bind(&notes).bind(format!("Verify and resolve {}",c.key)).execute(db.pool()).await.map_err(|e|e.to_string())?;
            Ok::<(),String>(())
        })?;

        if c.severity==Severity::P0 {
            rt.block_on(async{
                sqlx::query("update storyboard_story set status='Ready',updated_at=now() where id=$1").bind(&id)
                    .execute(db.pool()).await.map_err(|e|e.to_string())?;
                sqlx::query(
                    "update agent_work_item set kind='learn',learn_pattern_key=$2,special_instructions=$3,updated_at=now()
                     where story_id=$1 and state='Ready'"
                ).bind(&id).bind(&c.key).bind(&notes).execute(db.pool()).await.map_err(|e|e.to_string())?;
                Ok::<(),String>(())
            })?;
        } else {
            let batch=ensure_staging(db,rt)?;
            rt.block_on(async{
                sqlx::query("update storyboard_story set status='Batched',updated_at=now() where id=$1").bind(&id)
                    .execute(db.pool()).await.map_err(|e|e.to_string())?;
                sqlx::query(
                    "insert into forge_batch_item(batch_id,story_id,state,kind,learn_pattern_key)
                     values($1::uuid,$2,'Staged','learn',$3) on conflict(batch_id,story_id) do nothing"
                ).bind(batch).bind(&id).bind(&c.key).execute(db.pool()).await.map_err(|e|e.to_string())?;
                Ok::<(),String>(())
            })?;
        }
        Ok::<String,String>(id)
    })?
}

pub fn run_learn_pass(root:&Path, stale_after_minutes:i64)->Result<Option<String>,String>{
    let now=now_secs();
    let floor=now.saturating_sub(WINDOW_HOURS*3600);
    let since=read_anchor_secs(root).unwrap_or(floor).max(floor);
    let files=changed_files(root,since);
    let mut candidates=scan_silent_failures(&files);
    if let Some(stale)=stale_candidate(stale_after_minutes)? { candidates.push(stale); }
    let open=open_keys()?;
    candidates.retain(|c|!open.contains(&c.key));
    candidates.sort_by(|a,b|{
        match (&a.severity,&b.severity) {
            (Severity::P0,Severity::Normal)=>std::cmp::Ordering::Less,
            (Severity::Normal,Severity::P0)=>std::cmp::Ordering::Greater,
            _=>b.hit_count.cmp(&a.hit_count).then_with(||a.key.cmp(&b.key)),
        }
    });
    let filed=if let Some(c)=candidates.first(){Some(file_candidate(c)?)}else{None};
    write_anchor(root,candidates.first().map(|c|c.key.as_str()))?;
    Ok(filed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learn_sort_prefers_p0() {
        let mut v=vec![
            Candidate{pattern:"x".into(),key:"x".into(),severity:Severity::Normal,title:"x".into(),evidence:vec![],hit_count:99,first_seen:"".into(),last_seen:"".into()},
            Candidate{pattern:"stale-claim".into(),key:"stale-claim".into(),severity:Severity::P0,title:"x".into(),evidence:vec![],hit_count:1,first_seen:"".into(),last_seen:"".into()},
        ];
        v.sort_by(|a,b|match(&a.severity,&b.severity){(Severity::P0,Severity::Normal)=>std::cmp::Ordering::Less,(Severity::Normal,Severity::P0)=>std::cmp::Ordering::Greater,_=>b.hit_count.cmp(&a.hit_count)});
        assert_eq!(v[0].key,"stale-claim");
    }
}
