//! PROJECT.gantt — row positioning (TST-PROJECT-GANTT-004).
//!
//! Contract: dependency anchors keep their row indices end-to-end (source row on the source end, target row on the target end), and an unscheduled endpoint produces no anchor at all.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__004__row_positioning

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-004).
fn project_gantt_004__row_positioning() {
    let source = ProjectedTask {
        id: "a".into(),
        planned: NaiveDate::from_ymd_opt(2026, 10, 5).zip(NaiveDate::from_ymd_opt(2026, 10, 9)),
        descendant_span: None,
        deadline: None,
        partially_scheduled: false,
    };
    let target = ProjectedTask {
        id: "b".into(),
        planned: NaiveDate::from_ymd_opt(2026, 10, 10).zip(NaiveDate::from_ymd_opt(2026, 10, 14)),
        descendant_span: None,
        deadline: None,
        partially_scheduled: false,
    };
    let range = TimelineRange {
        start: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        end: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
    };
    let spec = timeline::spec("week");
    let anchors = timeline::link_anchors(&source, &target, 3, 7, range, spec).expect("a real link");
    assert_eq!(anchors.0 .1, 3, "the source anchor sits on the source row");
    assert_eq!(anchors.1 .1, 7, "the target anchor sits on the target row");
    let unscheduled = ProjectedTask {
        id: "c".into(),
        planned: None,
        descendant_span: None,
        deadline: None,
        partially_scheduled: false,
    };
    assert!(
        timeline::link_anchors(&source, &unscheduled, 3, 7, range, spec).is_none(),
        "unscheduled work has no fake anchor geometry"
    );
}
