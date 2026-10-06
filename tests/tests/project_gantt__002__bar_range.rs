//! PROJECT.gantt — bar range (TST-PROJECT-GANTT-002).
//!
//! Contract: a bar exists iff both dates are stored and ordered start <= finish; an inverted or one-sided span is refused as a bar and the one-sided case is flagged partially_scheduled.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__002__bar_range

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-002).
fn project_gantt_002__bar_range() {
    let page = page(
        vec![
            item(
                "ok",
                Some("p"),
                None,
                Some("2026-10-10"),
                Some("2026-10-14"),
                None,
                "ok",
            ),
            item(
                "inverted",
                Some("p"),
                None,
                Some("2026-10-14"),
                Some("2026-10-10"),
                None,
                "inverted",
            ),
            item(
                "partial",
                Some("p"),
                None,
                Some("2026-10-10"),
                None,
                None,
                "partial",
            ),
        ],
        vec![],
    );
    let schedule = timeline::project_schedule(&page, "p");
    let ok = schedule.tasks.iter().find(|t| t.id == "ok").unwrap();
    assert_eq!(
        ok.planned,
        NaiveDate::from_ymd_opt(2026, 10, 10).zip(NaiveDate::from_ymd_opt(2026, 10, 14))
    );
    assert!(!ok.partially_scheduled);
    let inverted = schedule.tasks.iter().find(|t| t.id == "inverted").unwrap();
    assert_eq!(
        inverted.planned, None,
        "an inverted span is not a bar — it is refused, never drawn backwards"
    );
    let partial = schedule.tasks.iter().find(|t| t.id == "partial").unwrap();
    assert_eq!(partial.planned, None);
    assert!(
        partial.partially_scheduled,
        "one-sided scheduling is flagged, not silently drawn"
    );
}
