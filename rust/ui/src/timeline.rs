//! Pure timeline/Gantt projection math.
//!
//! This module deliberately renders only facts the current WBS owns. A WBS
//! `due_at` is a deadline, not a task start, so leaves are milestones and parent
//! rows summarize descendant deadline spans. Planned durations are a separate
//! canonical WBS capability and must not be invented by the view.

use chrono::{Duration, NaiveDate};
use std::collections::BTreeSet;

use crate::model::{PortalProjectWorkItem, PortalProjectsPage, PortalWbsDependency};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedTask {
    pub id: String,
    /// Explicit work span, present only when both dates are stored.
    pub planned: Option<(NaiveDate, NaiveDate)>,
    /// Rollup over descendant planned spans, never persisted on the parent.
    pub descendant_span: Option<(NaiveDate, NaiveDate)>,
    pub deadline: Option<NaiveDate>,
    pub partially_scheduled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedSchedule {
    pub tasks: Vec<ProjectedTask>,
    pub links: Vec<PortalWbsDependency>,
}

/// Pure projection of canonical WBS facts. The renderer may choose its own row
/// order, but bars and links must come from this projection, not sample data.
pub fn project_schedule(page: &PortalProjectsPage, project_id: &str) -> ProjectedSchedule {
    let items: Vec<_> = page.items.iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id))
        .collect();
    let ids: BTreeSet<_> = items.iter().map(|item| item.id.as_str()).collect();
    let tasks = items.iter().map(|item| {
        let start = item.planned_start.as_deref().and_then(date);
        let finish = item.planned_finish.as_deref().and_then(date);
        let planned = match (start, finish) {
            (Some(start), Some(finish)) if start <= finish => Some((start, finish)),
            _ => None,
        };
        let mut dates = Vec::new();
        for child in &items {
            if child.id != item.id && is_descendant(child, item, &items) {
                if let (Some(start), Some(finish)) = (
                    child.planned_start.as_deref().and_then(date),
                    child.planned_finish.as_deref().and_then(date),
                ) {
                    if start <= finish {
                        dates.push((start, finish));
                    }
                }
            }
        }
        let descendant_span = dates.iter().map(|(start, _)| *start).min()
            .zip(dates.iter().map(|(_, finish)| *finish).max());
        ProjectedTask {
            id: item.id.clone(), planned, descendant_span,
            deadline: item.due_at.as_deref().and_then(date),
            partially_scheduled: start.is_some() ^ finish.is_some(),
        }
    }).collect();
    let links = page.dependencies.iter()
        .filter(|edge| edge.project_id == project_id
            && edge.kind == "finish_to_start"
            && ids.contains(edge.source_id.as_str())
            && ids.contains(edge.target_id.as_str()))
        .cloned().collect();
    ProjectedSchedule { tasks, links }
}

fn is_descendant(child: &PortalProjectWorkItem, ancestor: &PortalProjectWorkItem, items: &[&PortalProjectWorkItem]) -> bool {
    let mut parent = child.parent_id.as_deref();
    let mut visited = BTreeSet::new();
    while let Some(id) = parent {
        if id == ancestor.id { return true; }
        if !visited.insert(id) { return false; }
        parent = items.iter().find(|item| item.id == id).and_then(|item| item.parent_id.as_deref());
    }
    false
}

impl ProjectedSchedule {
    pub fn range(&self, fallback: &str, mode: &str) -> TimelineRange {
        let dates = self.tasks.iter().flat_map(|task| {
            [task.planned.map(|(start, _)| start),
             task.planned.map(|(_, finish)| finish), task.deadline]
                .into_iter().flatten().map(|date| date.to_string())
        }).collect::<Vec<_>>();
        range(dates.iter().map(String::as_str), fallback, mode)
    }
}

/// Connection anchors for a finish-to-start link. Both ends require a real
/// planned span; an unscheduled link stays in the graph without fake geometry.
pub fn link_anchors(
    source: &ProjectedTask, target: &ProjectedTask,
    source_row: i64, target_row: i64, range: TimelineRange, spec: TimelineSpec,
) -> Option<((i64, i64), (i64, i64))> {
    let (_, finish) = source.planned?;
    let (start, _) = target.planned?;
    Some(((x(finish, range, spec) + spec.pixels_per_day, source_row),
          (x(start, range, spec), target_row)))
}

pub fn planned_duration_days(start: NaiveDate, finish: NaiveDate) -> i64 {
    (finish - start).num_days() + 1
}

pub fn planned_bar_width(start: NaiveDate, finish: NaiveDate, spec: TimelineSpec) -> i64 {
    planned_duration_days(start, finish).max(1) * spec.pixels_per_day
}

/// Sort only siblings. The tree traversal remains the authoritative hierarchy;
/// this never rewrites `sort_order` in WBS.
pub fn sort_siblings(items: &mut Vec<&PortalProjectWorkItem>, key: &str, descending: bool) {
    if key.is_empty() { return; }
    items.sort_by(|a, b| {
        let ordering = match key {
            "title" => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            "start" => a.planned_start.cmp(&b.planned_start),
            "days" => {
                let days = |item: &PortalProjectWorkItem| item.planned_start.as_deref().and_then(date)
                    .zip(item.planned_finish.as_deref().and_then(date))
                    .map(|(start, finish)| planned_duration_days(start, finish));
                days(a).cmp(&days(b))
            }
            _ => std::cmp::Ordering::Equal,
        };
        let ordering = if descending { ordering.reverse() } else { ordering };
        ordering.then_with(|| a.order.cmp(&b.order)).then_with(|| a.id.cmp(&b.id))
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineSpec {
    pub key: &'static str,
    pub pixels_per_day: i64,
    pub margin_before: i64,
    pub margin_after: i64,
}

pub fn spec(mode: &str) -> TimelineSpec {
    match mode {
        "day" => TimelineSpec {
            key: "day",
            pixels_per_day: 34,
            margin_before: 7,
            margin_after: 14,
        },
        "month" => TimelineSpec {
            key: "month",
            pixels_per_day: 8,
            margin_before: 31,
            margin_after: 62,
        },
        _ => TimelineSpec {
            key: "week",
            pixels_per_day: 16,
            margin_before: 14,
            margin_after: 28,
        },
    }
}

pub fn date(value: &str) -> Option<NaiveDate> {
    let prefix = value.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

pub fn midnight_utc(value: &str) -> Option<String> {
    let date = date(value)?;
    date.and_hms_opt(0, 0, 0)
        .map(|value| value.and_utc().to_rfc3339())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl TimelineRange {
    pub fn days(self) -> i64 {
        (self.end - self.start).num_days().max(1)
    }

    pub fn width(self, spec: TimelineSpec) -> i64 {
        self.days() * spec.pixels_per_day
    }
}

pub fn range<'a>(
    values: impl IntoIterator<Item = &'a str>,
    fallback: &str,
    mode: &str,
) -> TimelineRange {
    let spec = spec(mode);
    let mut dates = values.into_iter().filter_map(date);
    let fallback = date(fallback)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("the epoch is a valid date"));
    let Some(first) = dates.next() else {
        return TimelineRange {
            start: fallback - Duration::days(spec.margin_before),
            end: fallback + Duration::days(spec.margin_after),
        };
    };

    let mut min = first;
    let mut max = first;
    for value in dates {
        min = min.min(value);
        max = max.max(value);
    }

    TimelineRange {
        start: min - Duration::days(spec.margin_before),
        end: max + Duration::days(spec.margin_after + 1),
    }
}

pub fn x(value: NaiveDate, range: TimelineRange, spec: TimelineSpec) -> i64 {
    (value - range.start).num_days() * spec.pixels_per_day
}

pub fn width_between(
    start: NaiveDate,
    end: NaiveDate,
    range: TimelineRange,
    spec: TimelineSpec,
) -> i64 {
    let start_x = x(start, range, spec);
    let end_x = x(end, range, spec);
    (end_x - start_x).abs().max(spec.pixels_per_day)
}

pub fn days(range: TimelineRange) -> Vec<NaiveDate> {
    (0..range.days())
        .map(|offset| range.start + Duration::days(offset))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_timeline_is_centered_on_the_server_supplied_day() {
        let range = range(std::iter::empty(), "2026-09-27", "week");
        assert_eq!(range.start.to_string(), "2026-09-13");
        assert_eq!(range.end.to_string(), "2026-10-25");
        assert_eq!(range.width(spec("week")), 42 * 16);
    }

    #[test]
    fn timeline_range_contains_real_dates_without_inventing_duration() {
        let range = range(
            ["2026-09-10T00:00:00Z", "2026-10-02T00:00:00Z"],
            "2026-09-27",
            "day",
        );
        assert_eq!(range.start.to_string(), "2026-09-03");
        assert_eq!(range.end.to_string(), "2026-10-17");
        assert_eq!(x(date("2026-09-10").unwrap(), range, spec("day")), 7 * 34);
    }

    #[test]
    fn due_dates_round_trip_as_date_semantics() {
        assert_eq!(
            midnight_utc("2026-11-03").as_deref(),
            Some("2026-11-03T00:00:00+00:00")
        );
    }

    #[test]
    fn canonical_projection_keeps_undated_and_deadline_only_work_unscheduled() {
        let item = |id: &str, parent: Option<&str>, start: Option<&str>, finish: Option<&str>, due: Option<&str>| PortalProjectWorkItem {
            id: id.into(), project_id: Some("p".into()), parent_id: parent.map(str::to_owned),
            planned_start: start.map(str::to_owned), planned_finish: finish.map(str::to_owned),
            due_at: due.map(str::to_owned), ..Default::default()
        };
        let page = PortalProjectsPage {
            items: vec![
                item("root", None, None, None, None),
                item("a", Some("root"), Some("2026-09-10"), Some("2026-09-12"), Some("2026-09-15T00:00:00Z")),
                item("b", Some("root"), None, None, Some("2026-09-20T00:00:00Z")),
                item("c", None, Some("2026-10-01"), None, None),
            ],
            dependencies: vec![PortalWbsDependency {
                project_id: "p".into(), source_id: "a".into(), target_id: "b".into(), kind: "finish_to_start".into(),
            }], ..Default::default()
        };
        let projection = project_schedule(&page, "p");
        let root = &projection.tasks[0];
        assert_eq!(root.descendant_span, Some((date("2026-09-10").unwrap(), date("2026-09-12").unwrap())));
        assert_eq!(projection.tasks[1].planned, root.descendant_span);
        assert_eq!(projection.tasks[1].deadline, date("2026-09-15"));
        assert_eq!(projection.tasks[2].planned, None);
        assert_eq!(projection.tasks[2].deadline, date("2026-09-20"));
        assert!(projection.tasks[3].partially_scheduled);
        assert_eq!(projection.links.len(), 1);
        assert!(link_anchors(&projection.tasks[1], &projection.tasks[2], 1, 2, projection.range("2026-09-27", "week"), spec("week")).is_none());
        assert!(projection.range("2026-09-27", "week").end > date("2026-10-01").unwrap());
        assert_eq!(planned_duration_days(date("2026-09-10").unwrap(), date("2026-09-12").unwrap()), 3);
        assert_eq!(planned_bar_width(date("2026-09-10").unwrap(), date("2026-09-10").unwrap(), spec("week")), 16);
    }

    #[test]
    fn sorting_changes_siblings_without_mutating_canonical_order() {
        let first = PortalProjectWorkItem { id: "a".into(), title: "Zulu".into(), order: Some(1), ..Default::default() };
        let second = PortalProjectWorkItem { id: "b".into(), title: "Alpha".into(), order: Some(2), ..Default::default() };
        let mut siblings = vec![&first, &second];
        sort_siblings(&mut siblings, "title", false);
        assert_eq!(siblings.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(first.order, Some(1));
        sort_siblings(&mut siblings, "title", true);
        assert_eq!(siblings[0].id, "a");
    }
}
