//! ONE-OFF: issue existing forms as Vault documents, the same way the portal's Save with issue does, so a converted form
//! carries a Vault copy of its current values. Ignored by default. Run it with:
//!   ISSUE_TARGET=prod ISSUE_ACTOR_APP_USER_ID=<lisa's app user id> ISSUE_FORM_IDS=<id,id,...> \
//!     cargo test -p test-harness --test forms_issue_batch -- --ignored --nocapture
//! The actor is the broker (her standing pre-signature needs her or a ROOT delegate); the principal is a ROOT delegate.
use db::{Database, DbTarget};
use model::IssueDocumentRequest;
use services::{ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};
use uuid::Uuid;

#[tokio::test]
#[ignore = "issues forms into the Vault; set ISSUE_TARGET, ISSUE_ACTOR_APP_USER_ID and ISSUE_FORM_IDS"]
async fn issue_the_listed_forms() {
    let target = match std::env::var("ISSUE_TARGET").as_deref() {
        Ok("prod") => DbTarget::Prod,
        _ => DbTarget::Dev,
    };
    let actor = std::env::var("ISSUE_ACTOR_APP_USER_ID").expect("ISSUE_ACTOR_APP_USER_ID is set");
    let ids = std::env::var("ISSUE_FORM_IDS").expect("ISSUE_FORM_IDS is set");
    let db = Database::connect_target(target).await.unwrap();
    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .expect("the production service infrastructure composes");
    let vault = web::composition::ServiceCatalog::new(db.clone(), infrastructure).vault();
    for form_id in ids.split(',').map(str::trim).filter(|id| !id.is_empty()) {
        let context = ServiceContext {
            actor: ServiceActor {
                id: Some(actor.clone()),
                kind: ServiceActorKind::User,
            },
            correlation_id: format!("issue-{form_id}"),
            causation_id: None,
            principal: Some(ServicePrincipal {
                app_user_id: actor.clone(),
                level: "ROOT".into(),
                role_codes: vec!["root".into()],
                account_type: "internal".into(),
                entitlement_codes: Vec::new(),
            }),
        };
        // The derived lines (the parties clause, the signature lines) are written on save in the portal. A converted form
        // was copied, not saved, so they are worked out here from its stored answers, and stored, before it is issued.
        let (stored,): (serde_json::Value,) =
            sqlx::query_as("select field_values from document_form_instance where id = $1::uuid")
                .bind(form_id)
                .fetch_one(db.pool())
                .await
                .expect("the form is read");
        let mut values: std::collections::BTreeMap<String, String> =
            serde_json::from_value(stored).expect("the answers are a map of text");
        let template_id: String = sqlx::query_scalar(
            "select template_id from document_form_instance where id = $1::uuid",
        )
        .bind(form_id)
        .fetch_one(db.pool())
        .await
        .expect("the template is read");
        model::forms_parties::apply_party_derivations(&template_id, &mut values);
        sqlx::query(
            "update document_form_instance set field_values = $2::jsonb where id = $1::uuid",
        )
        .bind(form_id)
        .bind(serde_json::to_value(&values).expect("the answers serialize"))
        .execute(db.pool())
        .await
        .expect("the derived lines are stored");
        let request = IssueDocumentRequest {
            command_id: Uuid::new_v4().to_string(),
            form_instance_id: form_id.to_owned(),
            actor_app_user_id: Some(actor.clone()),
            issued_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        let result = vault
            .issue_from_form_instance(&request, &context)
            .await
            .expect("the issue command runs");
        println!("{form_id}: {:?} {:?}", result.outcome, result.message);
    }
}
