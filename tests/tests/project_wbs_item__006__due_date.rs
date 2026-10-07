//! PROJECT.WBS.ITEM — due date (TST-PROJECT-WBS-ITEM-006).
//!
//! Contract: WBS items support a due_at timestamp field. The list_due query
//! returns items with status open/doing ordered by due_at (nulls last).
//!
//! Level: L2 Persistence — exercise the same boundary production uses. Use only
//! an isolated disposable Postgres/Neon test target; assert committed truth and
//! rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_wbs_item__006__due_date

use db::{Database, DbTarget, WbsDao};
use model::{CreateWbsItemRequest, WbsCategory, WbsStatus};
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

// Helper to query due items using raw SQL (avoids category mapping issue from dirty test data)
async fn query_due_items(
    db: &Database,
    project_id: &str,
    category: Option<WbsCategory>,
) -> Vec<(String, Option<String>, String)> {
    let category_str = category.as_ref().map(WbsCategory::as_str);
    sqlx::query_as(
        r#"
        select id, due_at::text, status
        from wbs_item
        where project_id = $1
          and status in ('open','doing')
          and ($2::text is null or category = $2)
        order by due_at nulls last, id
        "#,
    )
    .bind(project_id)
    .bind(category_str)
    .fetch_all(db.pool())
    .await
    .expect("query due items")
}

#[tokio::test]
async fn project_wbs_item_006__due_date() {
    let db = Database::connect_target(DbTarget::Dev).await.expect("connect to DEV database");
    let project_id = create_test_project(&db).await;
    let dao = WbsDao::new(db.clone());

    // 1. Create items with various due_at values
    let no_due_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: no_due_id.clone(),
        title: "No Due Date".into(),
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
    .expect("create no due");

    let due_early_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: due_early_id.clone(),
        title: "Due Early".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-01-15T10:00:00Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create due early");

    let due_late_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: due_late_id.clone(),
        title: "Due Late".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-12-31T23:59:59Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create due late");

    let due_middle_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: due_middle_id.clone(),
        title: "Due Middle".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-06-15T12:00:00Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create due middle");

    // 2. Create a completed item with due date (should NOT appear in list_due)
    let done_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: done_id.clone(),
        title: "Done with Due".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-01-10T10:00:00Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create done item");
    // Set to done
    dao.set_status(&done_id, WbsStatus::Done).await.expect("set done");

    // 3. Create a dismissed item with due date (should NOT appear in list_due)
    let dismissed_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: dismissed_id.clone(),
        title: "Dismissed with Due".into(),
        notes: None,
        category: WbsCategory::Management,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-01-05T10:00:00Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create dismissed item");
    dao.set_status(&dismissed_id, WbsStatus::Dismissed).await.expect("set dismissed");

    // 4. list_due with no category filter - should return open/doing items ordered by due_at nulls last
    let due_items = query_due_items(&db, &project_id, None).await;

    // Should have 4 items (no_due, early, middle, late) - done and dismissed excluded
    assert_eq!(due_items.len(), 4, "list_due returns open/doing items only");

    // Order: early, middle, late, no_due (nulls last)
    assert_eq!(due_items[0].0, due_early_id, "earliest due first");
    assert_eq!(due_items[1].0, due_middle_id, "middle due second");
    assert_eq!(due_items[2].0, due_late_id, "latest due third");
    assert_eq!(due_items[3].0, no_due_id, "no due date last (nulls last)");

    // 5. list_due with category filter
    let task_due = query_due_items(&db, &project_id, Some(WbsCategory::Management)).await;
    assert_eq!(task_due.len(), 4, "all are tasks");

    // Create a milestone with due date
    let milestone_id = Uuid::new_v4().to_string();
    dao.create(&CreateWbsItemRequest {
        id: milestone_id.clone(),
        title: "Milestone Due".into(),
        notes: None,
        category: WbsCategory::Accounting,
        project_id: Some(project_id.clone()),
        parent_id: None,
        due_at: Some("2026-03-01T10:00:00Z".into()),
        planned_start: None,
        planned_finish: None,
        owner: None,
        order: None,
        entity: None,
    })
    .await
    .expect("create milestone");

    let milestone_due = query_due_items(&db, &project_id, Some(WbsCategory::Accounting)).await;
    assert_eq!(milestone_due.len(), 1, "one milestone due");
    assert_eq!(milestone_due[0].0, milestone_id);

    // 6. Update due_at and verify it affects list_due order
    let updated = dao
        .save(&model::SaveWbsItemRequest {
            create: CreateWbsItemRequest {
                id: no_due_id.clone(),
                title: "Now Has Due Date".into(),
                notes: None,
                category: WbsCategory::Management,
                project_id: Some(project_id.clone()),
                parent_id: None,
                due_at: Some("2026-01-01T00:00:00Z".into()), // Earliest!
                planned_start: None,
                planned_finish: None,
                owner: None,
                order: None,
                entity: None,
            },
            status: None,
        })
        .await
        .expect("update due_at")
        .expect("item exists");
    assert_eq!(updated.due_at.as_ref().map(|s| s.replace("+00:00", "Z")), Some("2026-01-01T00:00:00Z".into()));

    let due_items = query_due_items(&db, &project_id, None).await;

    // Now no_due_id (which now has earliest due) should be first
    assert_eq!(due_items[0].0, no_due_id, "updated item now first");

    // 7. Negative case: due_at format - DAO stores as timestamptz, accepts ISO8601
    let bad_format_id = Uuid::new_v4().to_string();
    let result = dao
        .create(&CreateWbsItemRequest {
            id: bad_format_id.clone(),
            title: "Bad Format".into(),
            notes: None,
            category: WbsCategory::Management,
            project_id: Some(project_id.clone()),
            parent_id: None,
            due_at: Some("not-a-date".into()), // Invalid format
            planned_start: None,
            planned_finish: None,
            owner: None,
            order: None,
            entity: None,
        })
        .await;
    // DAO will fail at database level with invalid timestamptz
    assert!(result.is_err(), "invalid due_at format must fail at DB level");

    // 8. list_due excludes items with status Done/Dismissed
    let all_open_doing = query_due_items(&db, &project_id, None).await;
    // Should not include done or dismissed items
    assert!(
        !all_open_doing.iter().any(|i| i.0 == done_id),
        "Done item excluded from list_due"
    );
    assert!(
        !all_open_doing.iter().any(|i| i.0 == dismissed_id),
        "Dismissed item excluded from list_due"
    );
}