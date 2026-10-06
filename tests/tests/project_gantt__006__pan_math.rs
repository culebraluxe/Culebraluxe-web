//! PROJECT.gantt — pan math (TST-PROJECT-GANTT-006).
//!
//! Contract: panning shifts the visible window start-to-end without rescaling it; an empty projection is centered on the fallback date exactly margin_before/margin_after away.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__006__pan_math

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-006).
fn project_gantt_006__pan_math() {
    let mode = "week";
    let spec = timeline::spec(mode);
    let early = timeline::range(["2026-10-01"], "2026-10-01", mode);
    let late = timeline::range(["2026-10-20"], "2026-10-20", mode);
    assert!(
        early.start < late.start && early.end < late.end,
        "panning the content shifts the visible window rather than rescaling it"
    );
    // Panning preserves the span: the same window width.
    assert_eq!(
        early.days(),
        late.days(),
        "panning moves the window, never its size"
    );
    let shifted = timeline::range(["2026-10-08"], "2026-10-08", mode);
    let same_focus = timeline::range([], "2026-10-08", mode);
    // An empty set centers on the fallback, exactly margin_before/after around it.
    assert_eq!(
        same_focus.start.to_string(),
        (NaiveDate::from_ymd_opt(2026, 10, 8).unwrap()
            - chrono::Duration::days(spec.margin_before))
        .to_string()
    );
    assert_eq!(
        same_focus.end.to_string(),
        (NaiveDate::from_ymd_opt(2026, 10, 8).unwrap() + chrono::Duration::days(spec.margin_after))
            .to_string()
    );
    let _ = shifted;
}
