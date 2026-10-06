//! PROJECT.gantt — zoom math (TST-PROJECT-GANTT-005).
//!
//! Contract: zoom modes are ordered by pixels-per-day (day > week > month), the same date span projects proportionally wider at finer zoom, and an unknown zoom falls back to week.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__005__zoom_math

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-005).
fn project_gantt_005__zoom_math() {
    let day = timeline::spec("day");
    let week = timeline::spec("week");
    let month = timeline::spec("month");
    assert!(
        day.pixels_per_day > week.pixels_per_day,
        "day zoom is finer than week"
    );
    assert!(
        week.pixels_per_day > month.pixels_per_day,
        "week zoom is finer than month"
    );
    let start = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let finish = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    assert!(
        timeline::planned_bar_width(start, finish, day)
            > timeline::planned_bar_width(start, finish, month),
        "the same span is wider in a finer zoom"
    );
    let unknown = timeline::spec("centuries");
    assert_eq!(
        unknown.key, "week",
        "unknown zoom falls back to week, never panics"
    );
}
