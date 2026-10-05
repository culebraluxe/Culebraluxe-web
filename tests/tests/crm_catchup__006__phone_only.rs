//! CRM.CATCHUP — phone only resolution (TST-CRM-CATCHUP-006).
//!
//! Contract: when a catchup lead provides only a phone that resolves to an existing person,
//! the operation returns "resolved" with that person's id and creates an interaction.
//!
//! Level: L3 Composition — CRM domain projection test.
//! Harness: IntakeHarness, CrmHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__006__phone_only -- --ignored

use db::DbTarget;
use model::{CatchupLeadRequest, CatchupLeadResult, PersonIdentity, PersonIdentityKind};
use test_harness::{CrmHarness, IntakeHarness};

const HARNESS: &str = "IntakeHarness+CrmHarness/L3 Composition";

async fn connect_dev_intake() -> IntakeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match IntakeHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; IntakeHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

async fn connect_dev_crm() -> CrmHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match CrmHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; CrmHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); harnesses refuse PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-006); the file and the assay use it.
async fn crm_catchup_006__phone_only() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let intake = connect_dev_intake().await;
    let crm = connect_dev_crm().await;
    assert_eq!(
        intake.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the phone-only proof runs only on an isolated DEV target"
    );
    let ns = intake.namespace().to_string();
    let marker = format!("TST-CRMCATCHUP006-{ns}");

    // Create one person who owns the phone.
    let person = crm
        .seed_person(&format!("{marker}-existing"))
        .await
        .expect("existing person seeds");
    let phone = format!("1{:010}", ns.len() * 555555555 % 10_000_000_000);

    crm.attach(
        &person,
        PersonIdentity {
            kind: PersonIdentityKind::Phone,
            value: phone.clone(),
            source_system: None,
            is_primary: true,
        },
    )
    .await
    .expect("person gets phone");

    // Submit catchup lead with ONLY phone - it resolves to the existing person.
    let request = CatchupLeadRequest {
        name: format!("{marker}-incoming-name"),
        email: None,
        phone: Some(phone),
        message: Some("Phone only inquiry".to_string()),
    };

    let result = intake
        .submit_catchup(&request)
        .await
        .expect("catchup lead runs");

    // Contract: must return "resolved" with the existing person's id and an interaction.
    assert_eq!(
        result.status, "resolved",
        "{HARNESS}: matching phone must return resolved, got {}",
        result.status
    );
    assert_eq!(
        result.person_id,
        Some(person.clone()),
        "{HARNESS}: must return existing person's id"
    );
    assert!(
        result.interaction_id.is_some(),
        "{HARNESS}: interaction must be created"
    );

    // Verify the interaction was recorded.
    let interaction_id = result.interaction_id.unwrap();
    let interaction: Option<(String, String)> =
        sqlx::query_as("select person_id::text, title from interaction where id = $1::uuid")
            .bind(&interaction_id)
            .fetch_optional(crm.pool())
            .await
            .expect("read interaction");
    assert_eq!(
        interaction,
        Some((person, format!("Website inquiry from {}", request.name))),
        "{HARNESS}: interaction linked to correct person"
    );

    // Clean up.
    let removed = crm.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(removed, 1, "{HARNESS}: exactly one seeded person removed");
    assert_eq!(
        crm.leftover_count(&marker).await.expect("leftover count"),
        0
    );
}
