//! PROJECT.gantt — selection (TST-PROJECT-GANTT-008).
//!
//! Contract: the day list is exactly the visible selection: contiguous, starting at range.start, strictly before range.end, never empty even for a degenerate range.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__008__selection

use chrono::NaiveDate;
use ui::model::{PortalProjectWorkItem, PortalProjectsPage, PortalWbsDependency};
use ui::timeline::{self, ProjectedTask, TimelineRange, TimelineSpec};

fn item(
    id: &str,
    project: Option<&str>,
    parent: Option<&str>,
    start: Option<&str>,
    finish: Option<&str>,
    due: Option<&str>,
    title: &str,
) -> PortalProjectWorkItem {
    PortalProjectWorkItem {
        id: id.into(),
        project_id: project.map(str::to_owned),
        parent_id: parent.map(str::to_owned),
        planned_start: start.map(str::to_owned),
        planned_finish: finish.map(str::to_owned),
        due_at: due.map(str::to_owned),
        title: title.into(),
        ..Default::default()
    }
}

fn dep(project: &str, source: &str, target: &str, kind: &str) -> PortalWbsDependency {
    PortalWbsDependency {
        project_id: project.into(),
        source_id: source.into(),
        target_id: target.into(),
        kind: kind.into(),
    }
}

fn page(
    items: Vec<PortalProjectWorkItem>,
    dependencies: Vec<PortalWbsDependency>,
) -> PortalProjectsPage {
    PortalProjectsPage {
        items,
        dependencies,
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-008).
fn project_gantt_008__selection() {
    let range = TimelineRange {
        start: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        end: NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    };
    let days = timeline::days(range);
    assert_eq!(days.len() as i64, range.days(), "one cell per in-range day");
    assert_eq!(days.first().copied(), Some(range.start));
    assert!(days.last().copied().map(|d| d < range.end).unwrap_or(false));
    assert!(days.iter().all(|d| *d >= range.start && *d < range.end));
    let empty = TimelineRange {
        start: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        end: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
    };
    assert_eq!(
        empty.days(),
        1,
        "a degenerate range is a single day, not zero"
    );
}
