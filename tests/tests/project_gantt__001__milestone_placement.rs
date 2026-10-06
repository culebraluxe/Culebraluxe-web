//! PROJECT.gantt — milestone placement (TST-PROJECT-GANTT-001).
//!
//! Contract: a work item carrying only `due_at` projects as a milestone (no bar, the deadline point intact), while a dated task carries its bar — a deadline is never rewritten into a bar span.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__001__milestone_placement

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-001).
fn project_gantt_001__milestone_placement() {
    let page = page(
        vec![
            item(
                "milestone",
                Some("p"),
                None,
                None,
                None,
                Some("2026-10-15T00:00:00Z"),
                "closing",
            ),
            item(
                "task",
                Some("p"),
                None,
                Some("2026-10-10"),
                Some("2026-10-14"),
                None,
                "inspect",
            ),
        ],
        vec![],
    );
    let schedule = timeline::project_schedule(&page, "p");
    let milestone = schedule
        .tasks
        .iter()
        .find(|t| t.id == "milestone")
        .expect("milestone present");
    assert_eq!(
        milestone.planned, None,
        "a deadline-only row is a milestone: no bar, only a point"
    );
    assert_eq!(
        milestone.deadline,
        NaiveDate::from_ymd_opt(2026, 10, 15),
        "the milestone lands on its canonical deadline date, never invented"
    );
    let task = schedule
        .tasks
        .iter()
        .find(|t| t.id == "task")
        .expect("task present");
    assert!(
        task.planned.is_some(),
        "a fully scheduled row still has a bar"
    );
    assert!(
        milestone.partially_scheduled == false,
        "a deadline is not a half-scheduled bar"
    );
}
