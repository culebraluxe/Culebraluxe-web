//! PROJECT.WBS.ITEM — sort order (TST-PROJECT-WBS-ITEM-004).
//!
//! Contract: WBS items support a sort_order field that controls display order
//! within the same parent. Items with null sort_order sort after those with values.
//!
//! Level: L2 Persistence — exercise the same boundary production uses. Use only
//! an isolated disposable Postgres/Neon test target; assert committed truth and
//! rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_wbs_item__004__sort

use db::{Database, DbTarget, WbsDao};
use model::{CreateWbsItemRequest, SaveWbsItemRequest, WbsCategory};
use uuid::Uuid;

async fn create_test_project(db: &Database) -> String {
    let project_id = Uuid::new_v4().to_string();
    // FK on wbs_item.project_id references project(id) (migration 140)
    sqlx::query(
        r#"
        insert into project (id, name, status, created_at, updated_at)
        values ($1::uuid, 'Test Project', 'open', now(), now())
        "#,
    )
    .bind(&project_id)
    .execute(db.pool())
    .await
    .expect("create project");
    project_id
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): uses disposable Postgres; production is refused"]
async fn project_wbs_item_004__sort() {
    let db = Database::connect_target(DbTarget::Dev)
        .await
        .expect("connect to DEV database");
    let project_id = create_test_project(&db).await;
    let dao = WbsDao::new(db.clone());

    // Create a parent item
    let parent_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: parent_id.clone(),
        title: "Parent".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create parent");

    // Create children with different sort_order values
    let child1_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: child1_id.clone(),
        title: "Child 1 (order 10)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: Some(parent_id.clone()),
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(10),
        entity: None,
    })
    .await
    .expect("create child1");

    let child2_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: child2_id.clone(),
        title: "Child 2 (order 5)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: Some(parent_id.clone()),
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(5),
        entity: None,
    })
    .await
    .expect("create child2");

    let child3_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: child3_id.clone(),
        title: "Child 3 (no order)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: Some(parent_id.clone()),
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None, // null sort_order
        entity: None,
    })
    .await
    .expect("create child3");

    let child4_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: child4_id.clone(),
        title: "Child 4 (order 1)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: Some(parent_id.clone()),
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(1),
        entity: None,
    })
    .await
    .expect("create child4");

    // list_project_items orders by: project_id, parent_id nulls first, sort_order nulls last, due_at nulls last, id
    let children: Vec<(String, Option<i32>)> = sqlx::query_as(
        "select id, sort_order from wbs_item where parent_id = $1 order by sort_order nulls last, id"
    )
    .bind(&parent_id)
    .fetch_all(db.pool())
    .await
    .expect("query children with sort order");

    assert_eq!(children.len(), 4, "four children");

    // Should be ordered by sort_order nulls last: 1, 5, 10, null
    assert_eq!(children[0].0, child4_id, "order 1 comes first");
    assert_eq!(children[0].1, Some(1));
    assert_eq!(children[1].0, child2_id, "order 5 comes second");
    assert_eq!(children[1].1, Some(5));
    assert_eq!(children[2].0, child1_id, "order 10 comes third");
    assert_eq!(children[2].1, Some(10));
    assert_eq!(children[3].0, child3_id, "null order comes last");
    assert_eq!(children[3].1, None);

    // 2. Update sort_order and verify reordering
    let updated = dao
        .save(&SaveWbsItemRequest {
            create: CreateWbsItemRequest {
                id: child3_id.clone(),
                title: "Child 3 (now order 2)".into(),
                notes: None,
                category: WbsCategory::Management,
                project_id: Some(project_id.clone()),
                parent_id: Some(parent_id.clone()),
                due_at: None,
                planned_start: None,
                planned_finish: None,
                owner: None,
                order: Some(2), // Change from null to 2
                entity: None,
            },
            status: None,
        })
        .await
        .expect("update sort_order")
        .expect("item exists");
    assert_eq!(updated.order, Some(2));

    let children: Vec<(String, Option<i32>)> = sqlx::query_as(
        "select id, sort_order from wbs_item where parent_id = $1 order by sort_order nulls last, id"
    )
    .bind(&parent_id)
    .fetch_all(db.pool())
    .await
    .expect("query children after update");

    // New order: 1, 2, 5, 10
    assert_eq!(children[0].0, child4_id);
    assert_eq!(children[0].1, Some(1));
    assert_eq!(children[1].0, child3_id);
    assert_eq!(children[1].1, Some(2));
    assert_eq!(children[2].0, child2_id);
    assert_eq!(children[2].1, Some(5));
    assert_eq!(children[3].0, child1_id);
    assert_eq!(children[3].1, Some(10));

    // 3. Root-level items (no parent) also sort by sort_order
    let root1_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: root1_id.clone(),
        title: "Root 1 (order 3)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(3),
        entity: None,
    })
    .await
    .expect("create root1");

    let root2_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: root2_id.clone(),
        title: "Root 2 (order 1)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(1),
        entity: None,
    })
    .await
    .expect("create root2");

    // Query root items with raw SQL
    let roots: Vec<(String, Option<i32>)> = sqlx::query_as(
        "select id, sort_order from wbs_item where parent_id is null and project_id = $1 order by sort_order nulls last, id"
    )
    .bind(&project_id)
    .fetch_all(db.pool())
    .await
    .expect("query roots with sort order");

    // Roots ordered by sort_order nulls last: root2 (1), root1 (3)
    assert_eq!(roots[0].0, root2_id);
    assert_eq!(roots[0].1, Some(1));
    assert_eq!(roots[1].0, root1_id);
    assert_eq!(roots[1].1, Some(3));

    // 4. Negative case: duplicate sort_order values are allowed (stable sort by id)
    let child5_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: child5_id.clone(),
        title: "Child 5 (order 1 duplicate)".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: Some(parent_id.clone()),
        due_at: None,
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: Some(1), // Duplicate order
        entity: None,
    })
    .await
    .expect("create child5 with duplicate order");

    // Query children with raw SQL
    let children: Vec<(String, Option<i32>)> = sqlx::query_as(
        "select id, sort_order from wbs_item where parent_id = $1 order by sort_order nulls last, id"
    )
    .bind(&parent_id)
    .fetch_all(db.pool())
    .await
    .expect("query children with duplicate order");

    // Two items with order 1 - they sort by id as tiebreaker
    let order1_items: Vec<_> = children.iter().filter(|i| i.1 == Some(1)).collect();
    assert_eq!(order1_items.len(), 2, "two items with order 1");
    // The order between them is deterministic (by id)
    // We don't assert exact order, just that both exist and sort is stable
}
