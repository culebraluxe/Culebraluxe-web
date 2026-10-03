//! TECH COCKPIT and STORYBOARD — the story-board rules (ported from lib/storyboard-data.ts and lib/sorter-board.ts)
//! applied to the tech service's snapshot. Pure functions over JSON, tested below; the handlers live in portal_bridge.
//!
//! The four bins: a story's status puts it in OPEN, BACKLOG, CLOSED or NEXT VERSION. The KPIs count the bins; the
//! completion percent is the mean, over the story domains that have stories, of each domain's current-scope completion.
//! The Kanban (sorter) places each story in one column: engine (running, queued, Ready hand-off), batch (flight staging),
//! bench (the workbench), next version, backlog, open — first claim wins.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// A count as a whole number (the screen reads integers).
fn count(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// The bin a status belongs to.
pub fn lifecycle(status: &str) -> &'static str {
    match status {
        "Planned" | "Batched" => "backlog",
        "Complete" => "closed",
        "Deferred" => "next-version",
        _ => "open",
    }
}

fn prefix(id: &str) -> String {
    match id.split('-').next() {
        Some(part) if !part.is_empty() => format!("{part}-"),
        _ => id.to_owned(),
    }
}

/// NEXUS, MAIN, OPPS, SUPPORT, TECH — or UNCLASSIFIED.
pub fn story_domain(story: &Value) -> &'static str {
    let prefix = prefix(text(story, "id"));
    let workstream = text(story, "workstream");
    if ["PX-", "POLISH-", "PLAT-"].contains(&prefix.as_str())
        || ["PUBLIC", "CONTENT"].contains(&workstream)
    {
        return "MAIN";
    }
    if prefix == "AUTH-" {
        return "SUPPORT";
    }
    match text(story, "operatingSurface") {
        "NEXUS" => "NEXUS",
        "OPS" => "OPPS",
        "SUPPORT" => "SUPPORT",
        "TECH" => "TECH",
        _ => "UNCLASSIFIED",
    }
}

/// The group a story is shown under inside its bin.
pub fn story_subgroup(story: &Value) -> &'static str {
    let prefix = prefix(text(story, "id"));
    let workstream = text(story, "workstream");
    match story_domain(story) {
        "MAIN" => match prefix.as_str() {
            "PX-" => "PX / Public",
            "POLISH-" => "Public Site / Polish",
            "PLAT-" => "Property / Platform Data",
            _ => "Public / Content",
        },
        "NEXUS" => match (prefix.as_str(), workstream) {
            ("DOC-", _) => "Forms / DOC",
            ("INTAKE-", _) | (_, "CRM") => "CRM / Intake",
            (_, "PORTAL") => "Portal / Relationship",
            (_, "TXN") => "Deals / TXN",
            _ => "Other",
        },
        "OPPS" => match workstream {
            "ADMIN" => "Admin / Process",
            "CRM" => "CRM / Intake",
            "PORTAL" => "Portal / Ops",
            _ => "OPS / Operations",
        },
        "SUPPORT" => {
            if prefix == "AUTH-" {
                "AUTH / Security"
            } else {
                "Support / Ops"
            }
        }
        "TECH" => match prefix.as_str() {
            "ARCH-" => "ARCH",
            "MQ-" => "MQ MINI",
            "ALERT-" => "ALERTS",
            "WORKFLOW-" | "CRM-" => "WORKFLOW ENGINE",
            _ => "FRAMEWORKS",
        },
        _ => "Other",
    }
}

/// Mean completion of the current-scope (open or closed) rollup stories.
fn scope_completion(stories: &[&Value]) -> f64 {
    let participating: Vec<f64> = stories
        .iter()
        .filter(|story| story.get("rollup").and_then(Value::as_bool).unwrap_or(true))
        .filter(|story| matches!(lifecycle(text(story, "status")), "open" | "closed"))
        .map(|story| number(story, "completion"))
        .collect();
    if participating.is_empty() {
        return 0.0;
    }
    round1(participating.iter().sum::<f64>() / participating.len() as f64)
}

/// The board's completion percent: the mean over the domains that have stories.
pub fn completion_percent(stories: &[Value]) -> f64 {
    let per_domain: Vec<f64> = ["NEXUS", "MAIN", "OPPS", "SUPPORT", "TECH"]
        .iter()
        .map(|domain| {
            stories
                .iter()
                .filter(|story| story_domain(story) == *domain)
                .collect::<Vec<_>>()
        })
        .filter(|group| !group.is_empty())
        .map(|group| scope_completion(&group))
        .collect();
    if per_domain.is_empty() {
        return 0.0;
    }
    round1(per_domain.iter().sum::<f64>() / per_domain.len() as f64)
}

fn story_card(story: &Value) -> Value {
    json!({
        "id": text(story, "id"),
        "title": text(story, "title"),
        "priority": text(story, "priority"),
        "status": text(story, "status"),
        "completion": number(story, "completion"),
    })
}

/// The four bins, each grouped by subgroup (groups and stories sorted), with counts.
pub fn panels(stories: &[Value]) -> BTreeMap<&'static str, Value> {
    let mut out = BTreeMap::new();
    for bin in ["open", "backlog", "closed", "next-version"] {
        let mut grouped: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        let mut count = 0;
        for story in stories
            .iter()
            .filter(|story| lifecycle(text(story, "status")) == bin)
        {
            grouped
                .entry(story_subgroup(story))
                .or_default()
                .push(story);
            count += 1;
        }
        let groups: Vec<Value> = grouped
            .into_iter()
            .map(|(group, mut list)| {
                list.sort_by(|a, b| text(a, "id").cmp(text(b, "id")));
                json!({ "group": group, "stories": list.into_iter().map(story_card).collect::<Vec<_>>() })
            })
            .collect();
        out.insert(
            bin,
            json!({ "bucket": bin, "count": count, "groups": groups }),
        );
    }
    out
}

/// The KPIs over the bins.
pub fn kpis(stories: &[Value]) -> Value {
    let in_bin = |bin: &str| {
        stories
            .iter()
            .filter(|story| lifecycle(text(story, "status")) == bin)
            .count()
    };
    json!({
        "total": stories.len(),
        "open": in_bin("open"),
        "backlog": in_bin("backlog"),
        "blockedHold": stories.iter().filter(|story| matches!(text(story, "status"), "Blocked" | "Hold")).count(),
        "complete": in_bin("closed"),
        "nextVersion": in_bin("next-version"),
        "completionPercent": completion_percent(stories),
    })
}

pub const SORTER_COLUMNS: [(&str, &str); 5] = [
    ("backlog", "BACKLOG"),
    ("open", "OPEN"),
    ("bench", "WORK BENCH"),
    ("batch", "FLIGHT STAGING"),
    ("engine", "ENGINE RUN Q"),
];

/// The Kanban cards: each story in exactly one column, first claim wins (engine, batch, bench, next, backlog, open).
pub fn sorter_cards(snapshot: &Value, stories: &[Value]) -> Vec<Value> {
    let mut claimed = HashSet::new();
    let mut cards = Vec::new();
    let mut take =
        |column: &str, story: &Value, card_id: String, claim: String, kind: Option<&str>| {
            if !claimed.insert(claim) {
                return;
            }
            cards.push(json!({
                "id": card_id,
                "column": column,
                "title": text(story, "title"),
                "status": text(story, "status"),
                "priority": text(story, "priority"),
                "completion": number(story, "completion"),
                "kind": kind,
            }));
        };
    let list = |key: &str| {
        snapshot
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };

    for run in list("engineRuns") {
        let status = text(&run, "status");
        let stale = run.get("stale").and_then(Value::as_bool).unwrap_or(false);
        if stale || !matches!(status, "running" | "claimed" | "queued") {
            continue;
        }
        let story_id = text(&run, "storyId").to_owned();
        let attempts = run
            .get("attempts")
            .map(|a| a.to_string())
            .unwrap_or_default();
        let card = json!({ "title": text(&run, "title"), "status": status, "priority": "MEDIUM", "completion": 0 });
        take(
            "engine",
            &card,
            format!("{story_id}#{attempts}"),
            story_id,
            None,
        );
    }
    for item in list("queuedCards") {
        let story_id = text(&item, "storyId").to_owned();
        let card = json!({ "title": text(&item, "title"), "status": text(&item, "state"), "priority": "MEDIUM", "completion": 0 });
        take(
            "engine",
            &card,
            format!("{story_id}#queued"),
            story_id,
            None,
        );
    }
    for story in stories
        .iter()
        .filter(|story| text(story, "status") == "Ready")
    {
        let id = text(story, "id").to_owned();
        take("engine", story, format!("{id}#handoff"), id, None);
    }
    let kinds: HashMap<String, String> = list("stagingItems")
        .iter()
        .map(|item| {
            (
                text(item, "storyId").to_owned(),
                text(item, "kind").to_owned(),
            )
        })
        .collect();
    for story in stories
        .iter()
        .filter(|story| text(story, "status") == "Batched")
    {
        let id = text(story, "id").to_owned();
        let kind = kinds
            .get(&id)
            .map(String::as_str)
            .filter(|kind| !kind.is_empty());
        take("batch", story, id.clone(), id, kind);
    }
    for story in list("activeWork") {
        let id = text(&story, "id").to_owned();
        take("bench", &story, id.clone(), id, None);
    }
    for (bin, column) in [("backlog", "backlog"), ("open", "open")] {
        let mut in_bin: Vec<&Value> = stories
            .iter()
            .filter(|story| lifecycle(text(story, "status")) == bin)
            .collect();
        in_bin.sort_by(|a, b| {
            story_subgroup(a)
                .cmp(story_subgroup(b))
                .then(text(a, "id").cmp(text(b, "id")))
        });
        for story in in_bin {
            let id = text(story, "id").to_owned();
            take(column, story, id.clone(), id, None);
        }
    }
    cards
}

fn story_payload(story: &Value) -> Value {
    let keep = [
        "id",
        "workstream",
        "operatingSurface",
        "title",
        "priority",
        "status",
        "notes",
        "batch",
        "goal",
        "scope",
        "dependencies",
        "preconditions",
        "architectBrief",
        "contextRefs",
        "acceptanceCriteria",
        "postconditions",
        "completion",
        "updatedAt",
    ];
    let mut out = Map::new();
    for key in keep {
        out.insert(key.into(), story.get(key).cloned().unwrap_or(Value::Null));
    }
    for key in [
        "id",
        "workstream",
        "title",
        "priority",
        "status",
        "updatedAt",
    ] {
        if out[key].is_null() {
            out.insert(key.into(), json!(""));
        }
    }
    out.insert("completion".into(), json!(number(story, "completion")));
    Value::Object(out)
}

/// The Cockpit's whole payload, `{ tech: ... }`, from the tech service's snapshot.
pub fn cockpit(snapshot: &Value, forge_live: &Value, selected: Option<&str>, now: &str) -> Value {
    let list = |key: &str| {
        snapshot
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let executions: HashMap<String, Value> = list("executions")
        .into_iter()
        .map(|e| (text(&e, "storyId").to_owned(), e))
        .collect();
    let stories: Vec<Value> = list("stories")
        .into_iter()
        .map(|mut story| {
            let execution = executions
                .get(text(&story, "id"))
                .cloned()
                .unwrap_or(Value::Null);
            if let Some(object) = story.as_object_mut() {
                object.insert("execution".into(), execution);
            }
            story
        })
        .collect();
    let active = list("activeWork");
    let kpis = kpis(&stories);
    let selected_id = selected
        .filter(|id| stories.iter().any(|story| text(story, "id") == *id))
        .map(str::to_owned)
        .or_else(|| active.first().map(|s| text(s, "id").to_owned()))
        .or_else(|| stories.first().map(|s| text(s, "id").to_owned()));
    let selected_story = selected_id
        .as_deref()
        .and_then(|id| stories.iter().find(|story| text(story, "id") == id))
        .map(story_payload);
    let mut history: Vec<&Value> = stories
        .iter()
        .filter(|story| {
            story
                .pointer("/execution/latestRunAt")
                .and_then(Value::as_str)
                .is_some()
        })
        .collect();
    history.sort_by(|a, b| {
        let at = |s: &Value| {
            s.pointer("/execution/latestRunAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        at(b).cmp(&at(a))
    });
    let recent_history: Vec<Value> = history
        .into_iter()
        .take(4)
        .map(|story| {
            json!({
                "id": text(story, "id"),
                "title": text(story, "title"),
                "latestRunAt": story.pointer("/execution/latestRunAt"),
                "latestRunResult": story.pointer("/execution/latestRunResult"),
            })
        })
        .collect();
    let freshness = stories
        .iter()
        .map(|story| text(story, "updatedAt"))
        .max()
        .filter(|at| !at.is_empty())
        .unwrap_or(now)
        .to_owned();
    let ledger = snapshot
        .get("ledger")
        .filter(|v| !v.is_null())
        .map(|ledger| {
            json!({
                "totalAttempts": count(ledger, "totalAttempts"),
                "stories": count(ledger, "stories"),
                "completed": count(ledger, "completed"),
                "failed": count(ledger, "failed"),
                "interrupted": count(ledger, "interrupted"),
                "worstStoryId": ledger.get("worstStoryId"),
                "worstAttempts": ledger.get("worstAttempts"),
                "asOf": ledger.get("asOf"),
            })
        });
    json!({ "tech": {
        "ready": true,
        "totalStories": kpis["total"],
        "openCount": kpis["open"],
        "backlogCount": kpis["backlog"],
        "closedCount": kpis["complete"],
        "completionPercent": kpis["completionPercent"],
        "activeWork": active.iter().map(story_payload).collect::<Vec<_>>(),
        "selectedStory": selected_story,
        "selectedRuns": list("selectedRuns").into_iter().take(8).collect::<Vec<_>>(),
        "recorderInstanceId": snapshot.get("recorderInstanceId"),
        "hold": snapshot.get("hold"),
        "sorterCards": sorter_cards(snapshot, &stories),
        "sorterColumns": SORTER_COLUMNS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
        "engineRuns": list("engineRuns"),
        "queuedCards": list("queuedCards"),
        "engineReadOk": true,
        "queueReadOk": true,
        "ledger": ledger,
        "stagingFlight": snapshot.get("stagingFlight"),
        "recentFlights": list("recentFlights"),
        "recentHistory": recent_history,
        "freshness": freshness,
        "liveOps": forge_live,
    } })
}

/// The Storyboard's payload: KPIs and the four bins.
pub fn storyboard(snapshot: &Value) -> Value {
    let stories = snapshot
        .get("stories")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let panels = panels(&stories);
    json!({ "storyboard": {
        "kpis": kpis(&stories),
        "panels": {
            "open": panels["open"],
            "backlog": panels["backlog"],
            "closed": panels["closed"],
            "nextVersion": panels["next-version"],
        },
    } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story(id: &str, status: &str, completion: f64, surface: &str) -> Value {
        json!({ "id": id, "title": id, "status": status, "priority": "HIGH", "completion": completion,
                "workstream": "ADMIN", "operatingSurface": surface, "rollup": true, "updatedAt": "2026-09-27" })
    }

    #[test]
    fn statuses_fall_into_the_four_bins_and_the_kpis_count_them() {
        let stories = vec![
            story("FORGE-1", "In Progress", 50.0, "TECH"),
            story("FORGE-2", "Complete", 100.0, "TECH"),
            story("FORGE-3", "Planned", 0.0, "TECH"),
            story("FORGE-4", "Deferred", 0.0, "TECH"),
            story("OPS-1", "Hold", 20.0, "OPS"),
        ];
        let k = kpis(&stories);
        assert_eq!(
            (
                k["total"].as_u64(),
                k["open"].as_u64(),
                k["backlog"].as_u64()
            ),
            (Some(5), Some(2), Some(1))
        );
        assert_eq!(
            (
                k["complete"].as_u64(),
                k["nextVersion"].as_u64(),
                k["blockedHold"].as_u64()
            ),
            (Some(1), Some(1), Some(1))
        );
        // TECH current scope: (50 + 100) / 2 = 75; OPPS: 20; mean of the two domains = 47.5
        assert_eq!(k["completionPercent"].as_f64(), Some(47.5));
    }

    #[test]
    fn engine_queue_no_longer_uses_next_version_as_a_status_lane() {
        assert!(!SORTER_COLUMNS.iter().any(|(id, _)| *id == "next-version"));
        assert_eq!(SORTER_COLUMNS.len(), 5);
    }

    #[test]
    fn each_story_lands_in_one_kanban_column_engine_first() {
        let stories = vec![
            story("A-1", "Ready", 0.0, "TECH"),
            story("A-2", "Batched", 0.0, "TECH"),
            story("A-3", "Planned", 0.0, "TECH"),
            story("A-4", "In Progress", 10.0, "TECH"),
        ];
        let snapshot = json!({ "activeWork": [story("A-4", "In Progress", 10.0, "TECH")], "stagingItems": [{"storyId": "A-2", "kind": "fix"}] });
        let cards = sorter_cards(&snapshot, &stories);
        let column = |id: &str| {
            cards
                .iter()
                .find(|c| text(c, "id").starts_with(id))
                .map(|c| text(c, "column").to_owned())
        };
        assert_eq!(
            column("A-1").as_deref(),
            Some("engine"),
            "Ready is handed to the engine"
        );
        assert_eq!(column("A-2").as_deref(), Some("batch"));
        assert_eq!(column("A-3").as_deref(), Some("backlog"));
        assert_eq!(
            column("A-4").as_deref(),
            Some("bench"),
            "the workbench claims before open"
        );
        assert_eq!(cards.len(), 4, "no story twice");
    }
}
