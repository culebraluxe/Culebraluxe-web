//! Guest codes and provisioning against the DEV database (migration 219 applied there). Ignored by default; run with
//!   DATABASE_URL_DEV=... cargo test -p db --test guest_dev -- --ignored
//! Connects only to DEV, uses throwaway `@example.invalid` addresses, and deletes what it creates.

use db::{Database, DbTarget, GuestDao, GUEST_CODE_MAX_ATTEMPTS};
use domain::security::GuestClaim;

fn claim(provider: &str, subject: &str, email: &str, verified: bool) -> GuestClaim {
    GuestClaim {
        provider: provider.into(),
        subject: subject.into(),
        email: Some(email.into()),
        email_verified: verified,
        display_name: Some("Dev Guest".into()),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn guest_codes_and_provisioning_behave_on_dev() {
    let database = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = GuestDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("guest-{tag}@example.invalid");

    // Codes: only the newest is live, each attempt counts, five tries end it, one use.
    let first = uuid::Uuid::new_v4().to_string();
    dao.issue_code(&first, &email, "hash-1", Some("198.51.100.7"), 10)
        .await
        .unwrap();
    let second = uuid::Uuid::new_v4().to_string();
    dao.issue_code(&second, &email, "hash-2", Some("198.51.100.7"), 10)
        .await
        .unwrap();
    let history = dao
        .code_history(&email, Some("198.51.100.7"))
        .await
        .unwrap();
    assert_eq!(history.email_last_hour, 2);
    let attempt = dao.attempt_code(&email).await.unwrap().unwrap();
    assert_eq!(
        (attempt.id.as_str(), attempt.code_hash.as_str()),
        (second.as_str(), "hash-2")
    );
    for _ in 1..GUEST_CODE_MAX_ATTEMPTS {
        assert!(dao.attempt_code(&email).await.unwrap().is_some());
    }
    assert!(dao.attempt_code(&email).await.unwrap().is_none());
    assert!(dao.consume_code(&second).await.unwrap());
    assert!(!dao.consume_code(&second).await.unwrap());

    // Provisioning: an email-code guest, then Google with the same verified email joins it.
    let by_code = dao
        .provision(&claim("email-code", &email, &email, true), "guest")
        .await
        .unwrap();
    let google_sub = format!("dev-guest-{tag}");
    let by_google = dao
        .provision(&claim("google", &google_sub, &email, true), "Dev Guest")
        .await
        .unwrap();
    assert_eq!(by_code, by_google);
    assert_eq!(
        dao.provision(&claim("google", &google_sub, &email, true), "Dev Guest")
            .await
            .unwrap(),
        by_code,
        "a known identity returns its user"
    );
    let (account_type, roles, user_email): (String, Vec<String>, Option<String>) = sqlx::query_as(
        "select u.account_type, array_agg(r.code), u.email from app_user u \
         join app_user_role ur on ur.app_user_id = u.id join security_role r on r.id = ur.role_id \
         where u.id = $1::uuid group by u.account_type, u.email",
    )
    .bind(&by_code)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(account_type, "external");
    assert_eq!(roles, vec!["guest".to_string()]);
    assert_eq!(user_email.as_deref(), Some(email.as_str()));

    // An unverified email never links.
    let loose = dao
        .provision(
            &claim("google", &format!("dev-guest-loose-{tag}"), &email, false),
            "Loose",
        )
        .await
        .unwrap();
    assert_ne!(loose, by_code);

    // A staff address is never joined: the guest is created without an app_user email.
    let staff_email: Option<String> = sqlx::query_scalar(
        "select lower(email) from app_user where account_type = 'internal' and email is not null limit 1",
    )
    .fetch_optional(database.pool())
    .await
    .unwrap();
    let mut created = vec![by_code.clone(), loose];
    if let Some(staff_email) = staff_email {
        let staff_guest = dao
            .provision(
                &claim("email-code", &format!("staff-{tag}"), &staff_email, true),
                "Staff As Guest",
            )
            .await
            .unwrap();
        let (account_type, user_email): (String, Option<String>) =
            sqlx::query_as("select account_type, email from app_user where id = $1::uuid")
                .bind(&staff_guest)
                .fetch_one(database.pool())
                .await
                .unwrap();
        assert_eq!((account_type.as_str(), user_email), ("external", None));
        created.push(staff_guest);
    }

    for id in created {
        sqlx::query("delete from app_user where id = $1::uuid")
            .bind(id)
            .execute(database.pool())
            .await
            .unwrap();
    }
    sqlx::query("delete from guest_sign_in_code where email = $1")
        .bind(&email)
        .execute(database.pool())
        .await
        .unwrap();
}
