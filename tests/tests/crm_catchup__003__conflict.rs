//! CRM.CATCHUP — conflict resolution (TST-CRM-CATCHUP-003).
//!
//! Contract: when a catchup lead provides both email and phone that resolve to DIFFERENT people,
//! the operation returns "resolution_required" without creating a new person or interaction.
//!
//! Level: L3 Composition — CRM domain projection test.
//! Harness: IntakeHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__003__conflict -- --ignored

use model::{CatchupLeadRequest, CatchupLeadResult};
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-003); the file and the assay use it.
async fn crm_catchup_003__conflict() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let intake = connect_dev_intake().await;
    let crm = connect_dev_crm().await;
    assert_eq!(
        intake.database().target(),
        test_harness::DbTarget::Dev,
        "{HARNESS}: the conflict proof runs only on an isolated DEV target"
    );
    let ns = intake.namespace().to_string();
    let marker = format!("TST-CRMCATCHUP003-{ns}");

    // Create two different people: one owns the email, one owns the phone.
    let email = format!("conflict-{ns}@example.test");
    let phone = format!("1{:010}", ns.len() * 123456789 % 10_000_000_000);

    // Person A: owns the email
    let person_a = crm.seed_person(&format!("{marker}-person-a")).await
        .expect("person A seeds");
    crm.attach(&person_a, model::PersonIdentity {
        kind: model::PersonIdentityKind::Email,
        value: email.clone(),
        source_system: None,
        is_primary: true,
    }).await.expect("person A gets email");

    // Person B: owns the phone
    let person_b = crm.seed_person(&format!("{marker}-person-b")).await
        .expect("person B seeds");
    crm.attach(&person_b, model::PersonIdentity {
        kind: model::PersonIdentityKind::Phone,
        value: phone.clone(),
        source_system: None,
        is_primary: true,
    }).await.expect("person B gets phone");

    // Submit catchup lead with BOTH email and phone - they resolve to DIFFERENT people.
    let request = CatchupLeadRequest {
        name: format!("{marker}-new-person"),
        email: Some(email),
        phone: Some(phone),
        message: Some("Should trigger resolution_required".to_string()),
    };

    let result = intake.submit_catchup(&request).await
        .expect("catchup lead runs");

    // Contract: must return resolution_required, no person created, no interaction created.
    assert_eq!(
        result.status, "resolution_required",
        "{HARNESS}: conflicting identities must return resolution_required, got {}",
        result.status
    );
    assert!(result.person_id.is_none(), "{HARNESS}: no person_id on conflict");
    assert!(result.interaction_id.is_none(), "{HARNESS}: no interaction_id on conflict");

    // Verify no new person was created for this request.
    // The only persons with the marker prefix should be the two we seeded.
    let leftover = crm.leftover_count(&marker).await.expect("leftover count");
    assert_eq!(leftover, 2, "{HARNESS}: no new person created on conflict");

    // Clean up.
    let removed = crm.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(removed, 2, "{HARNESS}: exactly two seeded persons removed");
    assert_eq!(crm.leftover_count(&marker).await.expect("leftover count"), 0);
}