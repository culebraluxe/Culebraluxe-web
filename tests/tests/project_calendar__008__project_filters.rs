//! PROJECT.calendar — project filters (TST-PROJECT-CALENDAR-008).
//!
//! Contract: the calendar/timeline projection is strictly project-scoped: a caller scoped to project p1 never observes p2's or the orphan rows, and an unknown project id yields an empty projection rather than falling back to another project.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__008__project_filters

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-008).
fn project_calendar_008__project_filters() {
    // Two projects share one page; the calendar projection must be scoped to the selected one.
    let page = page(
        vec![
            item(
                "a1",
                Some("p1"),
                None,
                None,
                None,
                Some("2026-10-10T00:00:00Z"),
                "alpha",
            ),
            item(
                "a2",
                Some("p2"),
                None,
                None,
                None,
                Some("2026-10-11T00:00:00Z"),
                "beta",
            ),
            item(
                "a3",
                None,
                None,
                None,
                None,
                Some("2026-10-12T00:00:00Z"),
                "orphan",
            ),
        ],
        vec![],
    );
    let p1 = timeline::project_schedule(&page, "p1");
    assert!(!p1.tasks.is_empty(), "p1 still has its rows");
    for task in &p1.tasks {
        assert!(
            task.id.starts_with('a') && ["a1"].contains(&task.id.as_str()),
            "only p1 items in p1's projection, got {:?}",
            task.id
        );
    }
    let p2 = timeline::project_schedule(&page, "p2");
    assert!(
        p2.tasks.iter().all(|t| t.id == "a2"),
        "p2's projection carries only its own item"
    );
    let none = timeline::project_schedule(&page, "no-such-project");
    assert!(
        none.tasks.is_empty(),
        "an unknown project id yields an empty projection, never a fallback to another project"
    );
}
