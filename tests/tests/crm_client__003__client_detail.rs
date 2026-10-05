//! CRM.CLIENT — client detail (TST-CRM-CLIENT-003).
//!
//! Contract: the client detail endpoint returns a complete ClientDetail for a valid person_id,
//! including property interests, interactions, last_contact, next_action, and relationship_activity.
//! Returns None for non-existent or archived persons.
//!
//! Level: L3 Composition — CRM client detail.
//! Harness: ClientHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_client__003__client_detail -- --ignored

use db::DbTarget;
use model::{ClientDetail, ClientPropertyInterest};
use test_harness::ClientHarness;

const HARNESS: &str = "ClientHarness/L3 Composition";

async fn connect_dev() -> ClientHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ClientHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; ClientHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-003); the file and the assay use it.
async fn crm_client_003__client_detail() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the client detail proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCLIENT003-{ns}");

    // Seed a test client with full detail data.
    let pool = harness.pool();

    let person_id = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status, location, budget_min, budget_max, preferred_areas, property_types, priorities, timeline, notes)
         values ($1, 'buyer', 'active', 'Culebra, PR', 500000, 1000000, ARRAY['Culebra', 'Vieques'], ARRAY['condo', 'house'], ARRAY['ocean_view', 'privacy'], '3-6 months', 'Test notes')
         returning id::text"
    )
    .bind(format!("{marker}-detailed-client"))
    .fetch_one(pool)
    .await
    .expect("person seeds");

    // Add primary email and phone.
    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, is_primary) values ($1::uuid, 'email', $2, true)"
    )
    .bind(&person_id)
    .bind(format!("detail-{ns}@example.test"))
    .execute(pool)
    .await
    .expect("email");

    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, is_primary) values ($1::uuid, 'phone', $2, true)"
    )
    .bind(&person_id)
    .bind(format!("1{:010}", ns.len() * 111222333 % 10_000_000_000))
    .execute(pool)
    .await
    .expect("phone");

    // Create a test property for property_interest.
    let property_id = sqlx::query_scalar::<_, String>(
        "insert into property (name, location, list_price, bedrooms, property_type, status)
         values ($1, 'Culebra, PR', 750000, 2, 'condo', 'active') returning id::text",
    )
    .bind(format!("{marker}-test-property"))
    .fetch_one(pool)
    .await
    .expect("property seeds");

    // Add property interest.
    sqlx::query(
        "insert into property_interest (person_id, property_id, status, ranking)
         values ($1::uuid, $2::uuid, 'interested', 1)",
    )
    .bind(&person_id)
    .bind(&property_id)
    .execute(pool)
    .await
    .expect("property interest");

    // Add an interaction.
    let interaction_id = sqlx::query_scalar::<_, String>(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title, summary, source_system, source_external_id, source_metadata)
         values ($1::uuid, 'email', 'property_inquiry_submitted', 'inbound', now(), $2, $3, 'website', $4, $5)
         returning id::text"
    )
    .bind(&person_id)
    .bind("Property inquiry")
    .bind("Interested in ocean view condos")
    .bind(format!("{marker}-interaction-1"))
    .bind(serde_json::json!({"requestType": "property_information", "service": "market-analysis"}))
    .fetch_one(pool)
    .await
    .expect("interaction");

    // Add a task (for next_action).
    sqlx::query(
        "insert into task (title, detail, person_id, property_id, source_interaction_id, task_kind, priority, status, due_at)
         values ($1, $2, $3::uuid, $4::uuid, $5::uuid, 'human', 0, 'open', now() + interval '1 day')"
    )
    .bind("Follow up on property inquiry")
    .bind("Client wants ocean view condos in Culebra")
    .bind(&person_id)
    .bind(&property_id)
    .bind(&interaction_id)
    .execute(pool)
    .await
    .expect("task");

    // Refresh the materialized view so the new data is visible.
    sqlx::query("refresh materialized view concurrently mv_client_directory")
        .execute(pool)
        .await
        .expect("refresh mv");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Fetch detail for existing person - should return complete ClientDetail.
    // -----------------------------------------------------------------------------------------------------------
    let detail = harness
        .service()
        .detail(&person_id, &ctx)
        .await
        .expect("detail query runs")
        .expect("person exists");

    assert_eq!(detail.id, person_id, "{HARNESS}: correct person id");
    assert_eq!(
        detail.display_name,
        format!("{marker}-detailed-client"),
        "{HARNESS}: correct display_name"
    );
    assert_eq!(detail.role, "buyer", "{HARNESS}: correct role");
    assert_eq!(detail.status, "active", "{HARNESS}: correct status");
    assert_eq!(
        detail.location,
        Some("Culebra, PR".to_string()),
        "{HARNESS}: correct location"
    );
    assert!(detail.email.is_some(), "{HARNESS}: email present");
    assert!(detail.phone.is_some(), "{HARNESS}: phone present");
    assert_eq!(detail.budget_min, Some(500000.0), "{HARNESS}: budget_min");
    assert_eq!(detail.budget_max, Some(1000000.0), "{HARNESS}: budget_max");
    assert_eq!(
        detail.preferred_areas,
        vec!["Culebra", "Vieques"],
        "{HARNESS}: preferred_areas"
    );
    assert_eq!(
        detail.property_types,
        vec!["condo", "house"],
        "{HARNESS}: property_types"
    );
    assert_eq!(
        detail.priorities,
        vec!["ocean_view", "privacy"],
        "{HARNESS}: priorities"
    );
    assert_eq!(
        detail.timeline,
        Some("3-6 months".to_string()),
        "{HARNESS}: timeline"
    );
    assert_eq!(
        detail.notes,
        Some("Test notes".to_string()),
        "{HARNESS}: notes"
    );

    // Property interests.
    assert_eq!(
        detail.property_interests.len(),
        1,
        "{HARNESS}: one property interest"
    );
    let interest: &ClientPropertyInterest = &detail.property_interests[0];
    assert_eq!(
        interest.property_id, property_id,
        "{HARNESS}: correct property_id"
    );
    assert_eq!(
        interest.property_name,
        format!("{marker}-test-property"),
        "{HARNESS}: correct property_name"
    );
    assert_eq!(
        interest.status, "interested",
        "{HARNESS}: correct interest status"
    );

    // Interactions.
    assert_eq!(detail.interactions.len(), 1, "{HARNESS}: one interaction");
    assert_eq!(
        detail.interactions[0].id, interaction_id,
        "{HARNESS}: correct interaction id"
    );
    assert_eq!(
        detail.interactions[0].channel, "email",
        "{HARNESS}: correct channel"
    );
    assert_eq!(
        detail.interactions[0].event_type, "property_inquiry_submitted",
        "{HARNESS}: correct event_type"
    );

    // Last contact.
    assert!(
        detail.last_contact.is_some(),
        "{HARNESS}: last_contact present"
    );
    assert_eq!(
        detail.last_contact.as_ref().unwrap().channel,
        "email",
        "{HARNESS}: last_contact channel"
    );

    // Next action.
    assert!(
        detail.next_action.is_some(),
        "{HARNESS}: next_action present"
    );
    assert_eq!(
        detail.next_action.as_ref().unwrap().title,
        "Follow up on property inquiry",
        "{HARNESS}: next_action title"
    );

    // Relationship activity.
    assert!(
        detail.relationship_activity.is_some(),
        "{HARNESS}: relationship_activity present"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Fetch detail for non-existent person - should return None.
    // -----------------------------------------------------------------------------------------------------------
    let fake_id = "00000000-0000-0000-0000-000000000000";
    let result = harness
        .service()
        .detail(fake_id, &ctx)
        .await
        .expect("detail query runs");
    assert!(
        result.is_none(),
        "{HARNESS}: non-existent person returns None"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Fetch detail for archived person - should return None (archived_at is not null).
    // -----------------------------------------------------------------------------------------------------------
    let archived_id = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status, archived_at) values ($1, 'buyer', 'archived', now()) returning id::text"
    )
    .bind(format!("{marker}-archived"))
    .fetch_one(pool)
    .await
    .expect("archived person seeds");

    let result = harness
        .service()
        .detail(&archived_id, &ctx)
        .await
        .expect("detail query runs");
    assert!(result.is_none(), "{HARNESS}: archived person returns None");

    // Clean up.
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(
        removed, 2,
        "{HARNESS}: exactly two seeded persons removed (detailed + archived)"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
