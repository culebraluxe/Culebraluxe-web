//! PROJECT.gantt — dependency routing (TST-PROJECT-GANTT-009).
//!
//! Contract: the projection routes only same-project finish_to_start dependencies as links, and `items_link_broken` flags a dependent that starts on or before its predecessor finishes.
//!
//! Level: L0 Pure — the production projection math in `ui::timeline` / `ui::calendar`, no database,
//! no network. The production seams exercised are the same ones the browser screen calls:
//! `timeline::project_schedule`, `timeline::spec`, `timeline::range`/`x`/`link_anchors`/`link_broken`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__009__dependency_routing

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-009).
fn project_gantt_009__dependency_routing() {
    let mut page = page(
        vec![
            item(
                "pre",
                Some("p"),
                None,
                Some("2026-10-01"),
                Some("2026-10-05"),
                None,
                "pre",
            ),
            item(
                "post",
                Some("p"),
                None,
                Some("2026-10-06"),
                Some("2026-10-10"),
                None,
                "post",
            ),
            item("other", Some("p2"), None, None, None, None, "other"),
        ],
        vec![
            dep("p", "pre", "post", "finish_to_start"),
            dep("p", "pre", "other", "finish_to_start"),
            dep("p2", "other", "pre", "finish_to_start"),
            dep("p", "pre", "post", "relates_to"),
        ],
    );
    let schedule = timeline::project_schedule(&page, "p");
    assert_eq!(
        schedule.links.len(),
        1,
        "only finish_to_start edges inside the project survive"
    );
    assert_eq!(schedule.links[0].source_id, "pre");
    assert!(
        !timeline::items_link_broken(&page.items[0], &page.items[1],),
        "post starts the day pre ends+1: on time"
    );
    page.items[1].planned_start = Some("2026-10-04".into());
    assert!(
        timeline::items_link_broken(&page.items[0], &page.items[1]),
        "post starting before pre ends is broken"
    );
}
