//! Port of workflow_app/forge/forge-alerts.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub seq: u32,
    pub kind: String,
    pub verdict: Option<String>,
    pub story_id: String,
    pub node_id: Option<String>,
    pub task_id: Option<String>,
    pub reason: Option<String>,
    pub paths: Vec<String>,
    pub retry_hash: Option<String>,
    pub assignment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub code: &'static str,
    pub severity: &'static str,
    pub story_id: String,
    pub reason: String,
    pub event_seqs: Vec<u32>,
}

pub fn evaluate_alerts(events: &[TraceEvent]) -> Vec<Alert> {
    let mut out = Vec::new();
    out.extend(scope_denied(events));
    out.extend(sibling_collision(events));
    out.extend(unchanged_retry(events));
    out
}

fn scope_denied(events: &[TraceEvent]) -> Vec<Alert> {
    events
        .iter()
        .filter(|e| e.kind == "scope.check" && e.verdict.as_deref() == Some("deny"))
        .map(|e| Alert {
            code: "SCOPE_DENIED",
            severity: "hold-recommend",
            story_id: e.story_id.clone(),
            reason: e.reason.clone().unwrap_or_else(|| "candidate paths outside allowedScope".into()),
            event_seqs: vec![e.seq],
        })
        .collect()
}

fn sibling_collision(events: &[TraceEvent]) -> Vec<Alert> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut by_file: BTreeMap<&str, Vec<&TraceEvent>> = BTreeMap::new();
    for e in events {
        if !matches!(e.kind.as_str(), "git.commit" | "scope.check") { continue; }
        if e.verdict.as_deref() != Some("allow") { continue; }
        for p in &e.paths {
            by_file.entry(p.as_str()).or_default().push(e);
        }
    }
    let mut alerts = Vec::new();
    for (path, hits) in by_file {
        let owners: BTreeSet<String> = hits
            .iter()
            .map(|h| format!("{}:{}", h.node_id.as_deref().unwrap_or(""), h.assignment_id.as_deref().or(h.task_id.as_deref()).unwrap_or("")))
            .collect();
        if owners.len() < 2 { continue; }
        alerts.push(Alert {
            code: "SIBLING_FILE_COLLISION",
            severity: "hold-recommend",
            story_id: hits.first().map(|h| h.story_id.clone()).unwrap_or_default(),
            reason: format!("file {path} written by {} siblings", owners.len()),
            event_seqs: hits.iter().map(|h| h.seq).collect(),
        });
    }
    alerts
}

fn unchanged_retry(events: &[TraceEvent]) -> Vec<Alert> {
    use std::collections::BTreeMap;
    let mut seen: BTreeMap<&str, &TraceEvent> = BTreeMap::new();
    let mut alerts = Vec::new();
    for e in events {
        if e.kind != "hold" { continue; }
        let Some(hash) = e.retry_hash.as_deref() else { continue };
        if let Some(prior) = seen.get(hash) {
            alerts.push(Alert {
                code: "RETRY_UNCHANGED_INPUT",
                severity: "hold-recommend",
                story_id: e.story_id.clone(),
                reason: format!("retry hash {hash} already seen at seq {}", prior.seq),
                event_seqs: vec![prior.seq, e.seq],
            });
        } else {
            seen.insert(hash, e);
        }
    }
    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn denies_scope() {
        let events = [TraceEvent {
            seq: 1, kind: "scope.check".into(), verdict: Some("deny".into()),
            story_id: "s1".into(), node_id: Some("smith".into()), task_id: None,
            reason: None, paths: vec![], retry_hash: None, assignment_id: None,
        }];
        assert_eq!(evaluate_alerts(&events)[0].code, "SCOPE_DENIED");
    }
}
