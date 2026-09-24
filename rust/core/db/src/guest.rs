//! External guests of the public website: their emailed sign-in codes, and their provisioning as app_users.
//!
//! A GUEST IS AN ORDINARY APP_USER — account_type 'external', role 'guest' — reached through auth_identity like any
//! other sign-in. The security policy refuses every grant to an external account, so a guest cannot reach the portal.
//!
//! LINKING. A new sign-in with a VERIFIED email joins the existing EXTERNAL user with that email (so Google and an
//! email code are one guest). It never joins an internal (staff) user: when the address belongs to staff, the guest
//! is created without an app_user email, and the address stays on auth_identity.provider_email.
//!
//! A CODE IS COUNTED BEFORE IT IS CHECKED: `attempt_code` adds the attempt in the same statement that finds the live
//! code, so parallel guesses cannot share one, and `consume_code` marks it used only if nobody did first.

use crate::{Database, DbFailure, DbResult};
use domain::security::{GuestClaim, GuestCodeAttempt, GuestCodeHistory};

/// How many tries one code allows.
pub const GUEST_CODE_MAX_ATTEMPTS: i32 = 5;

pub struct GuestDao {
    db: Database,
}

impl GuestDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Codes sent in the last hour to this address and from this IP, and how long ago the last one for this address.
    pub async fn code_history(
        &self,
        email: &str,
        requester_ip: Option<&str>,
    ) -> DbResult<GuestCodeHistory> {
        let (email_last_hour, ip_last_hour, seconds_since_last_for_email) =
            sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                r#"
                select
                  (select count(*) from guest_sign_in_code
                    where email = $1 and created_at > now() - interval '1 hour'),
                  (select count(*) from guest_sign_in_code
                    where $2::text is not null and requester_ip = $2 and created_at > now() - interval '1 hour'),
                  (select extract(epoch from now() - max(created_at))::bigint from guest_sign_in_code
                    where email = $1)
                "#,
            )
            .bind(email)
            .bind(requester_ip)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("guest.code_history", &error))?;
        Ok(GuestCodeHistory {
            email_last_hour,
            ip_last_hour,
            seconds_since_last_for_email,
        })
    }

    /// Store a new code (its hash) and expire any older live code for the address, so only the newest email works.
    pub async fn issue_code(
        &self,
        id: &str,
        email: &str,
        code_hash: &str,
        requester_ip: Option<&str>,
        ttl_minutes: i32,
    ) -> DbResult<()> {
        let mut tx = self.db.begin("guest.issue_code").await?;
        sqlx::query(
            "update guest_sign_in_code set expires_at = now() \
             where email = $1 and consumed_at is null and expires_at > now()",
        )
        .bind(email)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.issue_code.expire_older", &error))?;
        sqlx::query(
            "insert into guest_sign_in_code (id, email, code_hash, requester_ip, expires_at) \
             values ($1::uuid, $2, $3, $4, now() + make_interval(mins => $5))",
        )
        .bind(id)
        .bind(email)
        .bind(code_hash)
        .bind(requester_ip)
        .bind(ttl_minutes)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.issue_code.insert", &error))?;
        tx.commit().await
    }

    /// Count one attempt against the address's live code and return it for checking; `None` when there is no live
    /// code (none sent, expired, used, or out of tries).
    pub async fn attempt_code(&self, email: &str) -> DbResult<Option<GuestCodeAttempt>> {
        let row = sqlx::query_as::<_, (String, String)>(
            r#"
            update guest_sign_in_code
            set attempts = attempts + 1
            where id = (
                select id from guest_sign_in_code
                where email = $1 and consumed_at is null and expires_at > now() and attempts < $2
                order by created_at desc
                limit 1
                for update
            )
            returning id::text, code_hash
            "#,
        )
        .bind(email)
        .bind(GUEST_CODE_MAX_ATTEMPTS)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.attempt_code", &error))?;
        Ok(row.map(|(id, code_hash)| GuestCodeAttempt { id, code_hash }))
    }

    /// Mark a code used. False when it was already used, so one code signs in once.
    pub async fn consume_code(&self, id: &str) -> DbResult<bool> {
        let row = sqlx::query_scalar::<_, String>(
            "update guest_sign_in_code set consumed_at = now() \
             where id = $1::uuid and consumed_at is null returning id::text",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.consume_code", &error))?;
        Ok(row.is_some())
    }

    /// The app_user behind a guest sign-in, provisioned on first sight; returns its id.
    ///
    /// Known identity → its user (last login stamped). Otherwise a verified email joins the EXTERNAL user with that
    /// email; failing that, a new external user with the `guest` role is created. The identity is then attached.
    /// `display_name` must already be non-empty (the service supplies a fallback).
    pub async fn provision(&self, claim: &GuestClaim, display_name: &str) -> DbResult<String> {
        let email = claim
            .email
            .as_deref()
            .filter(|_| claim.email_verified)
            .map(str::to_lowercase);
        let mut tx = self.db.begin("guest.provision").await?;

        let known = sqlx::query_scalar::<_, String>(
            "update auth_identity set last_login_at = now(), updated_at = now() \
             where provider = $1 and provider_subject = $2 returning app_user_id::text",
        )
        .bind(&claim.provider)
        .bind(&claim.subject)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.provision.known", &error))?;
        if let Some(app_user_id) = known {
            tx.commit().await?;
            return Ok(app_user_id);
        }

        let linked = match email.as_deref() {
            Some(email) => sqlx::query_scalar::<_, String>(
                "select id::text from app_user \
                 where lower(email) = $1 and account_type = 'external' order by created_at limit 1",
            )
            .bind(email)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("guest.provision.link", &error))?,
            None => None,
        };

        let app_user_id = match linked {
            Some(id) => id,
            None => {
                // The email goes on the app_user only when no user (staff included) already holds it.
                let id = sqlx::query_scalar::<_, String>(
                    r#"
                    insert into app_user (display_name, email, account_type)
                    values ($1, case when exists (select 1 from app_user where lower(email) = $2) then null else $2 end,
                            'external')
                    returning id::text
                    "#,
                )
                .bind(display_name)
                .bind(email.as_deref())
                .fetch_one(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("guest.provision.user", &error))?;
                let granted = sqlx::query(
                    "insert into app_user_role (app_user_id, role_id) \
                     select $1::uuid, r.id from security_role r \
                     where r.code = 'guest' and r.account_type = 'external' and r.active = true",
                )
                .bind(&id)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("guest.provision.role", &error))?;
                if granted.rows_affected() != 1 {
                    tx.rollback().await?;
                    return Err(DbFailure::configuration(
                        "guest.provision.role",
                        "the active external 'guest' role is missing",
                    ));
                }
                id
            }
        };

        // A concurrent first sign-in may have attached this identity already; theirs stands and is returned.
        sqlx::query(
            "insert into auth_identity (app_user_id, provider, provider_subject, provider_email, last_login_at) \
             values ($1::uuid, $2, $3, $4, now()) on conflict (provider, provider_subject) do nothing",
        )
        .bind(&app_user_id)
        .bind(&claim.provider)
        .bind(&claim.subject)
        .bind(claim.email.as_deref().map(str::to_lowercase))
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.provision.identity", &error))?;
        let app_user_id = sqlx::query_scalar::<_, String>(
            "select app_user_id::text from auth_identity where provider = $1 and provider_subject = $2",
        )
        .bind(&claim.provider)
        .bind(&claim.subject)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("guest.provision.read", &error))?;
        tx.commit().await?;
        Ok(app_user_id)
    }
}
