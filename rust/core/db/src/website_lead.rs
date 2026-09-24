//! Website leads, for their notification emails.
//!
//! EACH LEAD IS EMAILED ONCE. `claim_for_notice` stamps `notified_at` in the same statement that reads the lead, and
//! only for a lead that is recent and not yet stamped, so two concurrent requests cannot both send. A failed send
//! calls `release_notice`, so a retry can try again.

use crate::{Database, DbFailure, DbResult};
use domain::WebsiteLead;

pub struct WebsiteLeadDao {
    db: Database,
}

impl WebsiteLeadDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Claim a lead for notification: the lead, if it exists, arrived in the last hour and has not been notified.
    pub async fn claim_for_notice(&self, submission_id: &str) -> DbResult<Option<WebsiteLead>> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            with claimed as (
                update website_intake_submission s
                set notified_at = now()
                where s.id = $1::uuid
                  and s.notified_at is null
                  and s.created_at > now() - interval '1 hour'
                returning s.id, s.request_type, s.display_name, s.email, s.message, s.property_id
            )
            select c.id::text, c.request_type, c.display_name, c.email, c.message, p.name, p.slug
            from claimed c
            left join property p on p.id = c.property_id
            "#,
        )
        .bind(submission_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("website_lead.claim_for_notice", &error))?;
        Ok(row.map(
            |(id, request_type, display_name, email, message, property_name, property_slug)| {
                WebsiteLead {
                    id,
                    request_type,
                    display_name,
                    email,
                    message,
                    property_name,
                    property_slug,
                }
            },
        ))
    }

    /// Undo a claim after a failed send, so the lead can be notified on a retry.
    pub async fn release_notice(&self, submission_id: &str) -> DbResult<()> {
        sqlx::query("update website_intake_submission set notified_at = null where id = $1::uuid")
            .bind(submission_id)
            .execute(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("website_lead.release_notice", &error))?;
        Ok(())
    }
}
