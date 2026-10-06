//! PROJECT.gantt — date<->pixel inverse (TST-PROJECT-GANTT-007).
//!
//! Contract: x(date) is exactly the day offset from range.start scaled by pixels-per-day, and dividing it back reconstructs the same date — geometry is a pure function of the spec, never of layout state.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__007__date_pixel_inverse

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-007).
fn project_gantt_007__date_pixel_inverse() {
    let spec = timeline::spec("week");
    let range = TimelineRange {
        start: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        end: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
    };
    let day = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
    let px = timeline::x(day, range, spec);
    let days_from_start = (day - range.start).num_days();
    assert_eq!(
        px,
        days_from_start * spec.pixels_per_day,
        "x is the forward pixel of the date"
    );
    // Invert: the pixel offset measures whole days from the range start.
    let reconstructed = range.start + chrono::Duration::days(px / spec.pixels_per_day);
    assert_eq!(reconstructed, day, "date <-> pixel is a round trip");
    let start = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let finish = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let width = timeline::width_between(start, finish, range, spec);
    assert_eq!(
        width,
        (timeline::x(finish, range, spec) - timeline::x(start, range, spec))
            .abs()
            .max(spec.pixels_per_day),
        "width inverts offset difference"
    );
}
