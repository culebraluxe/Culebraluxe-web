use crate::{Database, DbFailure, DbResult};
use domain::{
    CatchupLeadRequest, CatchupLeadResult, WebsiteIntakeRequest, WebsiteIntakeResult,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
pub struct IntakeDao {
    db: Database,
}

impl IntakeDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn catchup_lead(
        &self,
        input: &CatchupLeadRequest,
    ) -> DbResult<CatchupLeadResult> {
        let mut tx = self.db.begin("intake.catchup_lead").await?;

        let email_owners = if let Some(email) = input.email.as_deref() {
            sqlx::query_scalar::<_, String>(
                r#"
                select distinct person_id::text
                  from person_identity
                 where identity_type = 'email' and identity_value = $1
                 order by person_id::text
                 limit 2
                "#,
            )
            .bind(email)
            .fetch_all(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("intake.catchup.email_owner", &error))?
        } else {
            Vec::new()
        };

        let phone_owners = if let Some(phone) = input.phone.as_deref() {
            sqlx::query_scalar::<_, String>(
                r#"
                select distinct person_id::text
                  from person_identity
                 where identity_type = 'phone' and identity_value = $1
                 order by person_id::text
                 limit 2
                "#,
            )
            .bind(phone)
            .fetch_all(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("intake.catchup.phone_owner", &error))?
        } else {
            Vec::new()
        };

        if email_owners.len() > 1 || phone_owners.len() > 1 {
            tx.rollback().await?;
            return Ok(CatchupLeadResult {
                status: "resolution_required".into(),
                person_id: None,
                interaction_id: None,
            });
        }

        let email_owner = email_owners.first().cloned();
        let phone_owner = phone_owners.first().cloned();
        if email_owner.is_some() && phone_owner.is_some() && email_owner != phone_owner {
            tx.rollback().await?;
            return Ok(CatchupLeadResult {
                status: "resolution_required".into(),
                person_id: None,
                interaction_id: None,
            });
        }

        let existing = email_owner.or(phone_owner);
        let (person_id, created) = if let Some(person_id) = existing {
            (person_id, false)
        } else {
            let person_id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                insert into person (id, display_name, role, status)
                values ($1::uuid, $2, 'buyer', 'new')
                "#,
            )
            .bind(&person_id)
            .bind(&input.name)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("intake.catchup.person", &error))?;

            if let Some(email) = input.email.as_deref() {
                sqlx::query(
                    r#"
                    insert into person_identity
                      (person_id, identity_type, identity_value, is_primary)
                    values ($1::uuid, 'email', $2, true)
                    "#,
                )
                .bind(&person_id)
                .bind(email)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("intake.catchup.email", &error))?;
            }
            if let Some(phone) = input.phone.as_deref() {
                sqlx::query(
                    r#"
                    insert into person_identity
                      (person_id, identity_type, identity_value, is_primary)
                    values ($1::uuid, 'phone', $2, true)
                    "#,
                )
                .bind(&person_id)
                .bind(phone)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("intake.catchup.phone", &error))?;
            }
            (person_id, true)
        };

        let submission_id = Uuid::new_v4().to_string();
        let interaction_id = sqlx::query_scalar::<_, String>(
            r#"
            insert into interaction (
              person_id, channel, event_type, direction, occurred_at,
              title, summary, source_system, source_external_id, source_metadata
            ) values (
              $1::uuid, 'website', 'lead_inquiry', 'inbound', now(),
              $2, $3, 'website', $4, $5
            )
            returning id::text
            "#,
        )
        .bind(&person_id)
        .bind(format!("Website inquiry from {}", input.name))
        .bind(input.message.as_deref())
        .bind(&submission_id)
        .bind(json!({"source":"website_inquiry","requestType":"general_enquiry"}))
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.catchup.interaction", &error))?;

        tx.commit().await?;
        Ok(CatchupLeadResult {
            status: if created { "created" } else { "resolved" }.into(),
            person_id: Some(person_id),
            interaction_id: Some(interaction_id),
        })
    }

    pub async fn website_intake(
        &self,
        input: &WebsiteIntakeRequest,
    ) -> DbResult<WebsiteIntakeResult> {
        let mut tx = self.db.begin("intake.website").await?;

        if let Some(property_id) = input.property_id.as_deref() {
            let exists = sqlx::query_scalar::<_, bool>(
                r#"
                select exists(
                  select 1 from property
                   where id = $1::uuid and archived_at is null
                )
                "#,
            )
            .bind(property_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("intake.website.property", &error))?;
            if !exists {
                tx.rollback().await?;
                return Ok(WebsiteIntakeResult {
                    accepted: false,
                    status: "invalid".into(),
                });
            }
        }

        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            insert into website_intake_submission (
              id, request_type, property_id, display_name, email, message
            ) values ($1::uuid, $2, $3::uuid, $4, $5, $6)
            on conflict (id) do nothing
            returning id::text
            "#,
        )
        .bind(&input.submission_id)
        .bind(&input.request_type)
        .bind(input.property_id.as_deref())
        .bind(&input.display_name)
        .bind(&input.email)
        .bind(input.message.as_deref())
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.receipt.insert", &error))?;

        let receipt = sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            select request_type, property_id::text, display_name, email, message,
                   status, processing_started_at::text, interaction_id::text
              from website_intake_submission
             where id = $1::uuid
             limit 1
            "#,
        )
        .bind(&input.submission_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.receipt.read", &error))?;

        if receipt.0 != input.request_type
            || receipt.1 != input.property_id
            || receipt.2 != input.display_name
            || receipt.3 != input.email
            || receipt.4 != input.message
        {
            tx.rollback().await?;
            return Ok(WebsiteIntakeResult {
                accepted: false,
                status: "invalid".into(),
            });
        }

        if inserted.is_none()
            && matches!(
                receipt.5.as_str(),
                "completed" | "rejected" | "resolution_required"
            )
        {
            tx.commit().await?;
            return Ok(WebsiteIntakeResult {
                accepted: true,
                status: "accepted".into(),
            });
        }

        let claim = sqlx::query_scalar::<_, String>(
            r#"
            update website_intake_submission
               set status = 'processing',
                   processing_started_at = now(),
                   updated_at = now()
             where id = $1::uuid
               and (
                 status = 'received'
                 or (
                   status = 'processing'
                   and processing_started_at <= now() - interval '15 minutes'
                 )
               )
             returning processing_started_at::text
            "#,
        )
        .bind(&input.submission_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.receipt.claim", &error))?;

        let Some(claim_token) = claim else {
            tx.commit().await?;
            return Ok(WebsiteIntakeResult {
                accepted: true,
                status: "accepted".into(),
            });
        };

        if let Some(existing_interaction) = sqlx::query_scalar::<_, String>(
            r#"
            select id::text from interaction
             where source_system = 'website' and source_external_id = $1
             limit 1
            "#,
        )
        .bind(&input.submission_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.duplicate", &error))?
        {
            transition_receipt(
                tx.connection(),
                &input.submission_id,
                &claim_token,
                "completed",
                Some(&existing_interaction),
            )
            .await?;
            tx.commit().await?;
            return Ok(WebsiteIntakeResult {
                accepted: true,
                status: "accepted".into(),
            });
        }

        let ownership = sqlx::query_as::<_, (String, bool)>(
            r#"
            select distinct pi.person_id::text, p.archived_at is not null as archived
              from person_identity pi
              join person p on p.id = pi.person_id
             where pi.identity_type = 'email'
               and lower(trim(pi.identity_value)) = lower(trim($1))
             order by pi.person_id::text
             limit 3
            "#,
        )
        .bind(&input.email)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.identity", &error))?;

        let active = ownership
            .iter()
            .filter(|(_, archived)| !*archived)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ownership.iter().any(|(_, archived)| *archived) || active.len() != 1 {
            transition_receipt(
                tx.connection(),
                &input.submission_id,
                &claim_token,
                "resolution_required",
                None,
            )
            .await?;
            tx.commit().await?;
            return Ok(WebsiteIntakeResult {
                accepted: true,
                status: "accepted".into(),
            });
        }
        let person_id = &active[0];

        let interaction_id = Uuid::new_v4().to_string();
        let event_type = match input.request_type.as_str() {
            "private_viewing" => "private_viewing_requested",
            "property_information" => "property_inquiry_submitted",
            _ => "general_enquiry_submitted",
        };
        let title = match input.request_type.as_str() {
            "private_viewing" => "Private viewing request",
            "property_information" => "Property information request",
            _ => "General enquiry",
        };
        let task_title = match input.request_type.as_str() {
            "private_viewing" => format!(
                "Follow up on private viewing request from {}",
                input.display_name
            ),
            "property_information" => format!(
                "Follow up on property inquiry from {}",
                input.display_name
            ),
            _ => format!("Follow up on general enquiry from {}", input.display_name),
        };

        let inserted_interaction = sqlx::query_scalar::<_, String>(
            r#"
            insert into interaction (
              id, person_id, property_id, channel, event_type, direction,
              occurred_at, title, summary, source_system, source_external_id,
              source_metadata
            ) values (
              $1::uuid, $2::uuid, $3::uuid, 'website', $4, 'inbound',
              now(), $5, $6, 'website', $7, $8
            )
            on conflict (source_system, source_external_id)
              where source_system is not null and source_external_id is not null
            do nothing
            returning id::text
            "#,
        )
        .bind(&interaction_id)
        .bind(person_id)
        .bind(input.property_id.as_deref())
        .bind(event_type)
        .bind(title)
        .bind(input.message.as_deref())
        .bind(&input.submission_id)
        .bind(json!({
            "requestType": input.request_type,
            "service": input.service,
        }))
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("intake.website.interaction", &error))?;

        let canonical_id = match inserted_interaction {
            Some(id) => {
                if let Some(property_id) = input.property_id.as_deref() {
                    sqlx::query(
                        r#"
                        insert into property_interest (person_id, property_id, status)
                        values ($1::uuid, $2::uuid, 'interested')
                        on conflict (person_id, property_id) do nothing
                        "#,
                    )
                    .bind(person_id)
                    .bind(property_id)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("intake.website.property_interest", &error)
                    })?;
                }

                sqlx::query(
                    r#"
                    insert into task (
                      title, detail, person_id, property_id, source_interaction_id,
                      task_kind, priority
                    ) values ($1, $2, $3::uuid, $4::uuid, $5::uuid, 'human', 0)
                    "#,
                )
                .bind(task_title)
                .bind(input.message.as_deref())
                .bind(person_id)
                .bind(input.property_id.as_deref())
                .bind(&id)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("intake.website.task", &error))?;
                id
            }
            None => sqlx::query_scalar::<_, String>(
                r#"
                select id::text from interaction
                 where source_system = 'website' and source_external_id = $1
                 limit 1
                "#,
            )
            .bind(&input.submission_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("intake.website.interaction.replay", &error))?,
        };

        transition_receipt(
            tx.connection(),
            &input.submission_id,
            &claim_token,
            "completed",
            Some(&canonical_id),
        )
        .await?;
        tx.commit().await?;

        Ok(WebsiteIntakeResult {
            accepted: true,
            status: "accepted".into(),
        })
    }
}

async fn transition_receipt(
    connection: &mut sqlx::PgConnection,
    submission_id: &str,
    claim_token: &str,
    status: &str,
    interaction_id: Option<&str>,
) -> DbResult<()> {
    let transitioned = sqlx::query_scalar::<_, String>(
        r#"
        update website_intake_submission
           set status = $3,
               processing_started_at = null,
               interaction_id = $4::uuid,
               updated_at = now()
         where id = $1::uuid
           and status = 'processing'
           and processing_started_at = $2::timestamptz
         returning id::text
        "#,
    )
    .bind(submission_id)
    .bind(claim_token)
    .bind(status)
    .bind(interaction_id)
    .fetch_optional(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("intake.website.receipt.transition", &error))?;

    if transitioned.is_none() {
        return Err(DbFailure::configuration(
            "intake.website.receipt.transition",
            "website intake receipt transition lost ownership",
        ));
    }
    Ok(())
}
