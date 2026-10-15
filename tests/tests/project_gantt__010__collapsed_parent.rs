//! PROJECT.gantt — collapsed parent (TST-PROJECT-GANTT-010).
//!
//! Contract: collapsing a parent row hides its whole subtree from the timeline while the
//! parent itself stays drawn (so it can be expanded again); expanding restores exactly the
//! rows that were hidden. The state change arrives through the production MVI intent
//! (`Msg::ProjectTimelineGroupToggled`, `selection.rs:205-211`) and the row list comes from
//! the same flattening the browser renders (`visible_rows`/`append_row`,
//! `web/ui/src/app/screens/projects/view/timeline.rs:672-717`, re-exported as
//! `projects::visible_rows`).
//!
//! Negative/fault cases: collapsing a different item hides nothing of this subtree,
//! collapsing an unknown id hides nothing, and a parent chain that loops terminates with
//! every id drawn exactly once instead of recursing forever (`append_row`'s visited set).
//!
//! Level: L0 Pure — no database, no network, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_gantt__010__collapsed_parent

use serde_json::json;
use ui::app::screen::{Screen, ScreenCtx};
use ui::app::screens::projects::{visible_rows, Msg, Model, Projects};

fn page() -> serde_json::Value {
    json!({ "projects": {
        "calendarToday": "2026-10-07",
        "projects": [
            { "id": "p1", "name": "Villa listing", "propertyId": "prop-1", "status": "doing" }
        ],
        "items": [
            { "id": "parent", "projectId": "p1", "title": "Launch", "status": "doing", "order": 1 },
            { "id": "child", "projectId": "p1", "parentId": "parent", "title": "Photos", "status": "open", "order": 2 },
            { "id": "grandchild", "projectId": "p1", "parentId": "child", "title": "Edit reel", "status": "open", "order": 3 },
            { "id": "sibling", "projectId": "p1", "title": "Sign", "status": "open", "order": 4 }
        ]
    } })
}

/// Open the production screen and answer its read with the fixture page.
fn opened() -> Model {
    let ctx = ScreenCtx::default();
    let (mut model, cmd) = Projects::init(&ctx);
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/portal/rust-ui/projects");
    Projects::update(&mut model, request.respond(Ok(page())), &ctx);
    model
}

/// `(id, depth, has_children)` for every visible row of the only project.
fn rows_of(model: &Model) -> Vec<(String, usize, bool)> {
    let page = model.read.loaded().expect("projects loaded");
    let project = page
        .projects
        .iter()
        .find(|project| project.id == "p1")
        .expect("fixture project");
    visible_rows(page, project)
        .into_iter()
        .map(|row| (row.item.id.clone(), row.depth, row.has_children))
        .collect()
}

fn ids(rows: &[(String, usize, bool)]) -> Vec<&str> {
    rows.iter().map(|(id, _, _)| id.as_str()).collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-GANTT-010).
fn project_gantt_010__collapsed_parent() {
    // 1. Expanded: the tree flattens depth-first, parent first, every row drawn once.
    let mut model = opened();
    assert_eq!(
        rows_of(&model),
        vec![
            ("parent".to_string(), 0, true),
            ("child".to_string(), 1, true),
            ("grandchild".to_string(), 2, false),
            ("sibling".to_string(), 0, false),
        ],
        "the hierarchy flattens to parent -> child -> grandchild, then the untouched sibling"
    );

    // 2. The production intent collapses the parent: the row list the view renders drops the
    //    child AND the grandchild, and keeps the parent itself drawn.
    let ctx = ScreenCtx::default();
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("parent".into()),
        &ctx,
    );
    assert!(
        model
            .read
            .loaded()
            .unwrap()
            .timeline_collapsed_items
            .contains("parent"),
        "the intent recorded the collapsed parent in the model"
    );
    assert_eq!(
        ids(&rows_of(&model)),
        vec!["parent", "sibling"],
        "a collapsed parent hides its whole subtree but stays visible itself"
    );

    // 3. Expanding restores exactly the hidden rows — nothing else moved.
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("parent".into()),
        &ctx,
    );
    assert_eq!(
        rows_of(&model),
        vec![
            ("parent".to_string(), 0, true),
            ("child".to_string(), 1, true),
            ("grandchild".to_string(), 2, false),
            ("sibling".to_string(), 0, false),
        ],
        "expanding the parent restores the subtree it hid"
    );

    // 4. Negative: collapsing a *different* item (the child) hides only its own subtree —
    //    the parent and the sibling are not collateral damage.
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("child".into()),
        &ctx,
    );
    assert_eq!(
        ids(&rows_of(&model)),
        vec!["parent", "child", "sibling"],
        "collapsing the child hides the grandchild only"
    );
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("child".into()),
        &ctx,
    );

    // 5. Negative: collapsing an id with no row behind it hides nothing — the collapse set
    //    is not a filter over rows by prefix or pattern, only by exact id.
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("ghost".into()),
        &ctx,
    );
    assert_eq!(
        ids(&rows_of(&model)),
        vec!["parent", "child", "grandchild", "sibling"],
        "a collapsed id that matches no row changes nothing"
    );
    Projects::update(
        &mut model,
        Msg::ProjectTimelineGroupToggled("ghost".into()),
        &ctx,
    );
    assert!(
        model
            .read
            .loaded()
            .unwrap()
            .timeline_collapsed_items
            .is_empty(),
        "the model ends with an empty collapse set"
    );

    // 6. Fault: a malformed hierarchy cannot hang or duplicate rows. `parent` now points
    //    back at `grandchild` (a cycle) and a self-parented item is added. The looped
    //    component is unreachable from the roots, the self-parented item can only reach
    //    itself, and `append_row`'s visited set guarantees every drawn id appears exactly
    //    once — the flattening terminates instead of recursing forever.
    {
        let mut page: ui::model::PortalProjectsPage =
            serde_json::from_value(page()).expect("fixture page decodes");
        page.items
            .iter_mut()
            .find(|item| item.id == "parent")
            .expect("parent exists")
            .parent_id = Some("grandchild".into());
        page.items.push(ui::model::PortalProjectWorkItem {
            id: "ouroboros".into(),
            project_id: Some("p1".into()),
            parent_id: Some("ouroboros".into()),
            title: "self-parented".into(),
            status: "open".into(),
            ..Default::default()
        });
        let project = page.projects[0].clone();
        let rows: Vec<String> = visible_rows(&page, &project)
            .into_iter()
            .map(|row| row.item.id.clone())
            .collect();
        assert_eq!(
            rows,
            vec!["sibling".to_string()],
            "only the reachable root draws; the cycle and the self-parented item neither hang nor leak rows"
        );
    }
}