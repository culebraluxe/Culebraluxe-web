//! DB.SCHEMA — enum/check vocabulary matches Rust enum (TST-DB-SCHEMA-004).
//!
//! CONTRACT. Where a Rust enum is the source of truth for a `text` column, the database
//! CHECK constraint on that column must speak exactly the same vocabulary as the enum.
//! Two drifts are forbidden, and the test reports each in the direction it occurs:
//!
//! - a Rust variant whose `as_str()` is absent from the CHECK constraint — production can
//!   write a value the database refuses (a real INSERT dies at the persistence boundary); and
//! - a CHECK value that is not a Rust variant — the database accepts a value the Rust reader
//!   (`TryFrom<&str>`, the exact conversion the DAO runs on read) cannot parse.
//!
//! The Rust side of each pair is read from the production enum itself (`as_str()`), never
//! retyped as a string list, and the database side is the live CHECK constraint read from
//! `pg_constraint`. The pairs are the docsign/notification boundary the Rust DAOs write
//! (`db::document_sign`, `db::signer`, `db::email`): every writer binds `enum.as_str()` and
//! every reader runs `Enum::try_from(row.column.as_str())`.
//!
//! Level: L2 Persistence — the production snapshot/catalogue boundary against an isolated
//! disposable DEV/Neon target. The harness refuses PRODUCTION before any socket is opened,
//! and the one live write happens inside a transaction the harness only knows how to roll
//! back.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__004__enum_check_vocabulary_matches_rust_enum -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use std::collections::BTreeSet;

use db::DbTarget;
use model::{
    DocumentSigningMode, EmailMessageKind, EmailMessageStatus, SignatureFieldType,
    SignatureRecipientRole, SignatureRequestStatus, SignerState,
};
use sqlx::PgConnection;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// One Rust-enum ↔ database-CHECK pair under test.
///
/// `rust_vocabulary` reads the variant list from the production enum via `as_str()`;
/// `rust_accepts` runs the exact `TryFrom<&str>` conversion the DAO's read path uses, so a
/// CHECK value the reader cannot parse is caught even when `as_str()` and `TryFrom` disagree.
struct EnumCheck {
    /// The table owning the column.
    table: &'static str,
    /// The single `text` column carrying the CHECK constraint.
    column: &'static str,
    /// The Rust enum's name, for the failure message.
    enum_name: &'static str,
    /// The enum's vocabulary, read from the type (`as_str()`), never retyped.
    rust_vocabulary: fn() -> Vec<&'static str>,
    /// Whether the DAO's read path (`TryFrom<&str>`) accepts a value.
    rust_accepts: fn(&str) -> bool,
}

/// The pairs the docsign/notification persistence boundary relies on. Every table/column here
/// is written by a Rust DAO that binds `enum.as_str()`, and read back through `TryFrom<&str>`.
const ENUM_CHECKS: &[EnumCheck] = &[
    EnumCheck {
        table: "signature_request",
        column: "status",
        enum_name: "SignatureRequestStatus",
        rust_vocabulary: signature_request_status_vocabulary,
        rust_accepts: signature_request_status_accepts,
    },
    EnumCheck {
        table: "signature_recipient_state",
        column: "state",
        enum_name: "SignerState",
        rust_vocabulary: signer_state_vocabulary,
        rust_accepts: signer_state_accepts,
    },
    EnumCheck {
        table: "signature_field",
        column: "field_type",
        enum_name: "SignatureFieldType",
        rust_vocabulary: signature_field_type_vocabulary,
        rust_accepts: signature_field_type_accepts,
    },
    EnumCheck {
        table: "document_sign_request",
        column: "signing_mode",
        enum_name: "DocumentSigningMode",
        rust_vocabulary: document_signing_mode_vocabulary,
        rust_accepts: document_signing_mode_accepts,
    },
    EnumCheck {
        table: "signature_envelope_recipient",
        column: "recipient_role",
        enum_name: "SignatureRecipientRole",
        rust_vocabulary: signature_recipient_role_vocabulary,
        rust_accepts: signature_recipient_role_accepts,
    },
    EnumCheck {
        table: "email_message",
        column: "message_kind",
        enum_name: "EmailMessageKind",
        rust_vocabulary: email_message_kind_vocabulary,
        rust_accepts: email_message_kind_accepts,
    },
    EnumCheck {
        table: "email_message",
        column: "status",
        enum_name: "EmailMessageStatus",
        rust_vocabulary: email_message_status_vocabulary,
        rust_accepts: email_message_status_accepts,
    },
];

fn signature_request_status_vocabulary() -> Vec<&'static str> {
    use SignatureRequestStatus::{self, *};
    [
        Requested, Sent, Viewed, Signed, Completed, Declined, Voided, Expired, Error,
    ]
    .into_iter()
    .map(SignatureRequestStatus::as_str)
    .collect()
}

fn signature_request_status_accepts(value: &str) -> bool {
    SignatureRequestStatus::try_from(value).is_ok()
}

fn signer_state_vocabulary() -> Vec<&'static str> {
    use SignerState::{self, *};
    [
        Pending, Notified, Viewed, InProgress, Completed, Declined, Expired, Revoked,
    ]
    .into_iter()
    .map(SignerState::as_str)
    .collect()
}

fn signer_state_accepts(value: &str) -> bool {
    SignerState::try_from(value).is_ok()
}

fn signature_field_type_vocabulary() -> Vec<&'static str> {
    use SignatureFieldType::{self, *};
    [
        Signature, Initials, Name, Email, Date, Text, Number, Checkbox, Radio, Dropdown,
    ]
    .into_iter()
    .map(SignatureFieldType::as_str)
    .collect()
}

fn signature_field_type_accepts(value: &str) -> bool {
    SignatureFieldType::try_from(value).is_ok()
}

fn document_signing_mode_vocabulary() -> Vec<&'static str> {
    use DocumentSigningMode::{self, *};
    [Sequential, Parallel]
        .into_iter()
        .map(DocumentSigningMode::as_str)
        .collect()
}

fn document_signing_mode_accepts(value: &str) -> bool {
    DocumentSigningMode::try_from(value).is_ok()
}

fn signature_recipient_role_vocabulary() -> Vec<&'static str> {
    use SignatureRecipientRole::{self, *};
    [Signer, Approver]
        .into_iter()
        .map(SignatureRecipientRole::as_str)
        .collect()
}

fn signature_recipient_role_accepts(value: &str) -> bool {
    use SignatureRecipientRole::*;
    [Signer, Approver]
        .into_iter()
        .any(|role| role.as_str() == value)
}

fn email_message_kind_vocabulary() -> Vec<&'static str> {
    use EmailMessageKind::{self, *};
    [
        SignatureInvitation,
        SignatureReminder,
        SignatureCompleted,
        SignatureDeclined,
    ]
    .into_iter()
    .map(EmailMessageKind::as_str)
    .collect()
}

fn email_message_kind_accepts(value: &str) -> bool {
    EmailMessageKind::try_from(value).is_ok()
}

fn email_message_status_vocabulary() -> Vec<&'static str> {
    use EmailMessageStatus::{self, *};
    [Queued, Sending, Sent, Failed, Dead, Cancelled]
        .into_iter()
        .map(EmailMessageStatus::as_str)
        .collect()
}

fn email_message_status_accepts(value: &str) -> bool {
    EmailMessageStatus::try_from(value).is_ok()
}

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Every single-quoted literal in a `pg_get_constraintdef` rendering.
///
/// Postgres normalises `column IN ('a','b')` to `column = ANY (ARRAY['a'::text,'b'::text])`,
/// so splitting on the quote delimiter and keeping alternate fields extracts the vocabulary
/// without depending on which of the two spellings the migration used. Enum vocabulary is
/// identifier-shaped and carries no escaped quote, which is the only case this needs to serve.
fn quoted_literals(definition: &str) -> Vec<String> {
    definition
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// The symmetric difference between the Rust enum vocabulary and the database CHECK vocabulary.
///
/// Empty means the two vocabularies agree. Non-empty names each drift in the direction it
/// occurs, so a single-sided comparison that answered "match" for everything cannot pass.
fn vocabulary_drift(enum_name: &str, rust: &[&str], database: &[String]) -> Vec<String> {
    let rust_set: BTreeSet<&str> = rust.iter().copied().collect();
    let database_set: BTreeSet<&str> = database.iter().map(String::as_str).collect();

    let mut drift: Vec<String> = Vec::new();
    for value in rust_set.difference(&database_set) {
        drift.push(format!(
            "{enum_name}: Rust variant `{value}` is absent from the database CHECK vocabulary"
        ));
    }
    for value in database_set.difference(&rust_set) {
        drift.push(format!(
            "{enum_name}: database CHECK value `{value}` is not a Rust {enum_name} variant"
        ));
    }
    drift
}

/// Read the vocabulary of the single-column CHECK constraint on `table.column` from the catalogue.
///
/// Matching on `pg_attribute.attnum = ANY(conkey)` rather than on a constraint name is deliberate:
/// inline `check (...)` constraints carry auto-generated names, and the contract is about the
/// column's vocabulary, not about what Postgres happened to call the constraint.
async fn read_check_vocabulary(
    connection: &mut PgConnection,
    table: &str,
    column: &str,
) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "select pg_get_constraintdef(c.oid) \
         from pg_constraint c \
         join pg_class t on t.oid = c.conrelid \
         join pg_namespace n on n.oid = t.relnamespace \
         join pg_attribute a on a.attrelid = t.oid and a.attnum = any(c.conkey) \
         where c.contype = 'c' and n.nspname = 'public' \
           and t.relname = $1 and a.attname = $2 and array_length(c.conkey, 1) = 1 \
         order by c.conname",
    )
    .bind(table)
    .bind(column)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| format!("read CHECK constraints on {table}.{column}: {error}"))?;

    if rows.is_empty() {
        return Err(format!(
            "no single-column CHECK constraint on {table}.{column} — the vocabulary is unenforced"
        ));
    }

    let mut values: BTreeSet<String> = BTreeSet::new();
    for (definition,) in rows {
        values.extend(quoted_literals(&definition));
    }
    Ok(values.into_iter().collect())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-004); the file and the assay use it.
async fn db_schema_004__enum_check_vocabulary_matches_rust_enum() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the enum/check vocabulary proof runs only on an isolated DEV target"
    );

    // 1. The negative control: the comparator discriminates, so a comparator that answered
    //    "match" for everything could not pass the real comparison below.
    let both_directions = vocabulary_drift(
        "Control",
        &["alpha", "beta"],
        &["alpha".to_owned(), "gamma".to_owned()],
    );
    assert_eq!(
        both_directions.len(),
        2,
        "{HARNESS}: the comparator reports a Rust-only and a database-only drift; got {both_directions:?}"
    );
    assert!(
        vocabulary_drift("Control", &["alpha"], &["alpha".to_owned()]).is_empty(),
        "{HARNESS}: identical vocabularies compare clean — the comparator is not a constant"
    );

    // 2. Read every live CHECK vocabulary and compare it to the Rust enum's own vocabulary.
    let mut tx = dev.begin().await.expect("begin catalogue-read transaction");
    let mut drift: Vec<String> = Vec::new();
    for check in ENUM_CHECKS {
        let database_values =
            match read_check_vocabulary(tx.connection(), check.table, check.column).await {
                Ok(values) => values,
                Err(error) => {
                    drift.push(format!("{}: {error}", check.enum_name));
                    continue;
                }
            };

        // 2a. Both directions: Rust variants absent from the CHECK, CHECK values absent from Rust.
        drift.extend(vocabulary_drift(
            check.enum_name,
            &(check.rust_vocabulary)(),
            &database_values,
        ));

        // 2b. Round-trip: every value the CHECK permits must parse through the DAO's reader.
        for value in &database_values {
            if !(check.rust_accepts)(value) {
                drift.push(format!(
                    "{}: database CHECK on {}.{} allows `{value}` but TryFrom<&str> refuses it",
                    check.enum_name, check.table, check.column
                ));
            }
        }
    }
    tx.rollback().await.expect("roll back the catalogue read");

    assert!(
        drift.is_empty(),
        "{HARNESS}: enum/check vocabulary drift on DEV:\n  {}",
        drift.join("\n  ")
    );

    // 3. The live enforcement boundary: an out-of-vocabulary value is refused by the CHECK,
    //    a Rust-produced value is accepted and readable — all inside a rolled-back transaction,
    //    so no canonical row survives. `email_message` is the FK-free pair, so the fault is the
    //    vocabulary and nothing else.
    let dedupe_key = format!("tst-db-schema-004-{}", dev.namespace());

    let mut rejected_tx = dev
        .begin()
        .await
        .expect("begin rejected-insert transaction");
    let rejected = sqlx::query(
        "insert into email_message (message_kind, recipient_email, template_key, dedupe_key, status) \
         values ('signature_invitation', 'harness@example.test', 'tst_db_schema_004', $1, 'not_a_real_status')",
    )
    .bind(&dedupe_key)
    .execute(rejected_tx.connection())
    .await;
    assert!(
        rejected.is_err(),
        "{HARNESS}: email_message.status CHECK must refuse `not_a_real_status`; the insert succeeded"
    );
    rejected_tx
        .rollback()
        .await
        .expect("roll back rejected insert");

    let mut accepted_tx = dev
        .begin()
        .await
        .expect("begin accepted-insert transaction");
    let queued = EmailMessageStatus::Queued.as_str();
    sqlx::query(
        "insert into email_message (message_kind, recipient_email, template_key, dedupe_key, status) \
         values ('signature_invitation', 'harness@example.test', 'tst_db_schema_004', $1, $2)",
    )
    .bind(&dedupe_key)
    .bind(queued)
    .execute(accepted_tx.connection())
    .await
    .expect("the Rust enum's as_str() value is accepted by the CHECK");
    let stored: (String,) =
        sqlx::query_as("select status from email_message where dedupe_key = $1")
            .bind(&dedupe_key)
            .fetch_one(accepted_tx.connection())
            .await
            .expect("the accepted row is readable inside the transaction");
    assert_eq!(
        stored.0, queued,
        "{HARNESS}: the stored status round-trips to the Rust value that was written"
    );
    accepted_tx
        .rollback()
        .await
        .expect("roll back accepted insert");
}
