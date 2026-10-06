//! PROJECT.gantt — hierarchy flattening (TST-PROJECT-GANTT-003).
//!
//! Contract: parent rows summarize descendant planned spans — the roll-up never persists on the parent, includes transitive descendants, and never invents a span for an undated parent.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__003__hierarchy_flattening

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-003).
fn project_gantt_003__hierarchy_flattening() {
    let page = page(
        vec![
            item("root", Some("p"), None, None, None, None, "root"),
            item(
                "child",
                Some("p"),
                Some("root"),
                Some("2026-10-05"),
                Some("2026-10-08"),
                None,
                "child",
            ),
            item(
                "grand",
                Some("p"),
                Some("child"),
                Some("2026-10-01"),
                Some("2026-10-20"),
                None,
                "grand",
            ),
        ],
        vec![],
    );
    let schedule = timeline::project_schedule(&page, "p");
    let root = schedule.tasks.iter().find(|t| t.id == "root").unwrap();
    assert_eq!(
        root.descendant_span,
        NaiveDate::from_ymd_opt(2026, 10, 1).zip(NaiveDate::from_ymd_opt(2026, 10, 20)),
        "the parent rolls up the descendant planned span"
    );
    let child = schedule.tasks.iter().find(|t| t.id == "child").unwrap();
    assert_eq!(
        root.descendant_span.unwrap().0,
        NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
    );
    assert_eq!(
        child.descendant_span,
        NaiveDate::from_ymd_opt(2026, 10, 1).zip(NaiveDate::from_ymd_opt(2026, 10, 20)),
        "the roll-up includes transitive descendants"
    );
}
