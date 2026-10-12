//! ONE-OFF: issue one form and send it for signature through the same luxesign.send command the portal's Send uses, with
//! the signer's address set to the recipient named here. Ignored by default. Run it with:
//!   SEND_TARGET=dev SEND_ACTOR_APP_USER_ID=<lisa's app user id> SEND_FORM_ID=<id> SEND_SIGNER_NAME=<name> \
//!   SEND_RECIPIENT_EMAIL=<address> cargo test -p test-harness --test forms_send_test_path -- --ignored --nocapture
//! This is a PATH test: the signer block is addressed to the recipient, not to the person named on the form.
use db::{Database, DbTarget};
use model::IssueDocumentRequest;
use serde_json::{json, Value};
use services::{CommandRequest, ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};
use uuid::Uuid;

#[tokio::test]
#[ignore = "issues one form and sends it for signature; set SEND_TARGET, SEND_ACTOR_APP_USER_ID, SEND_FORM_ID, SEND_SIGNER_NAME, SEND_RECIPIENT_EMAIL"]
async fn send_the_form_for_signature_to_the_recipient() {
    // An unknown target is a refusal, never a silent fall back to DEV: the send is real on PROD.
    let target = match std::env::var("SEND_TARGET").as_deref() {
        Ok("prod") => DbTarget::Prod,
        Ok("dev") => DbTarget::Dev,
        other => panic!("SEND_TARGET must be \"dev\" or \"prod\" (got {other:?})"),
    };
    let actor = std::env::var("SEND_ACTOR_APP_USER_ID").expect("SEND_ACTOR_APP_USER_ID is set");
    let form_id = std::env::var("SEND_FORM_ID").expect("SEND_FORM_ID is set");
    let signer_name = std::env::var("SEND_SIGNER_NAME").expect("SEND_SIGNER_NAME is set");
    let recipient = std::env::var("SEND_RECIPIENT_EMAIL").expect("SEND_RECIPIENT_EMAIL is set");

    let db = Database::connect_target(target).await.unwrap();
    let vault = web::composition::ServiceCatalog::new(
        db.clone(),
        web::service_bootstrap::production_service_infrastructure(&db)
            .await
            .expect("the production service infrastructure composes"),
    )
    .vault();
    let harness = web::service_harness::ServiceHarness::new(
        db.clone(),
        web::service_bootstrap::production_service_infrastructure(&db)
            .await
            .expect("the production service infrastructure composes"),
    )
    .expect("the service harness composes");

    let context = ServiceContext {
        actor: ServiceActor {
            id: Some(actor.clone()),
            kind: ServiceActorKind::User,
        },
        correlation_id: format!("send-{form_id}"),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: actor.clone(),
            level: "ROOT".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: Vec::new(),
        }),
    };

    // The derived lines are worked out from the stored answers and stored before issue, as the issuance runner does.
    let (stored,): (Value,) =
        sqlx::query_as("select field_values from document_form_instance where id = $1::uuid")
            .bind(&form_id)
            .fetch_one(db.pool())
            .await
            .expect("the form is read");
    let template_id: String =
        sqlx::query_scalar("select template_id from document_form_instance where id = $1::uuid")
            .bind(&form_id)
            .fetch_one(db.pool())
            .await
            .expect("the template is read");
    let mut values: std::collections::BTreeMap<String, String> =
        serde_json::from_value(stored).expect("the answers are a map of text");
    model::forms_parties::apply_party_derivations(&template_id, &mut values);
    sqlx::query("update document_form_instance set field_values = $2::jsonb where id = $1::uuid")
        .bind(&form_id)
        .bind(serde_json::to_value(&values).expect("the answers serialize"))
        .execute(db.pool())
        .await
        .expect("the derived lines are stored");

    let issue = vault
        .issue_from_form_instance(
            &IssueDocumentRequest {
                command_id: Uuid::new_v4().to_string(),
                form_instance_id: form_id.clone(),
                actor_app_user_id: Some(actor.clone()),
                issued_at: Some(chrono::Utc::now().to_rfc3339()),
            },
            &context,
        )
        .await
        .expect("the issue command runs");
    println!("issue: {:?} {:?}", issue.outcome, issue.message);
    let issued = vault
        .issued_for_form_instance(&form_id, &context)
        .await
        .expect("the issued document is read")
        .expect("the form has an issued document");

    // The envelope: the seller block, addressed to the recipient. Lisa's pre-signed broker block is not an envelope
    // recipient, so nothing else is sent.
    let input = json!({
        "transactionDocumentId": issued.document_id,
        "recipients": [{
            "role": "signer",
            "name": signer_name,
            "email": recipient,
            "signerOrder": 1,
            "signingStep": 1,
            "executionRole": "SELLER",
            "executionSlotId": "SELLER:1",
        }],
        "subject": format!("{signer_name} Listing Agreement"),
        "message": null,
        "signingMode": "parallel",
        "expiresAt": null,
        "placement": { "kind": "template" },
        "copyTo": [],
        "absentRoles": [],
    });
    let Value::Object(input) = input else {
        unreachable!("json! of an object literal is an object")
    };
    let result = harness
        .execute_command(
            &CommandRequest {
                command_id: Uuid::new_v4().to_string(),
                command_type: "luxesign.send".into(),
                aggregate_type: "transaction_document".into(),
                aggregate_id: Some(issued.document_id.clone()),
                requested_at: chrono::Utc::now().to_rfc3339(),
                input,
            },
            &context,
        )
        .await;
    println!("send: {result:?}");
}
