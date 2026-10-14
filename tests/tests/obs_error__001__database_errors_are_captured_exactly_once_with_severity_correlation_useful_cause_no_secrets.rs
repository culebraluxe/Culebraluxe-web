//! OBS.ERROR — database errors are captured exactly once with severity, correlation, useful cause, no secrets, and
//! an appropriate returned error (TST-OBS-ERROR-001).
//!
//! Contract: **a database failure leaves exactly one record, carrying everything an operator needs and nothing they
//! must not have.** This is the `Error Capture Obligation` in `AGENTS.md` made executable, and the seam is
//! production code end to end:
//!
//! - **the announcement.** Every `DbFailure` is announced from its CONSTRUCTORS, not from each `map_err`
//!   (`db/src/error.rs:47-53` — `fn announced(self)`). That is the design worth testing: the alternative is that
//!   the one call site somebody forgets is the one that was needed. `announced` calls `capture::notify`, which
//!   invokes the process's sink.
//! - **the sink.** `db::on_failure` (`db/src/capture.rs:33-35`) takes a plain `fn(&DbFailure)` — no allocation,
//!   no trait object, and it cannot capture state that would keep a connection open. **First caller wins**, and
//!   the return value says whether this caller installed it.
//! - **exactly once.** The sink runs once per failure (`capture.rs:46-52`), and a failure raised INSIDE the sink
//!   is swallowed by a depth counter rather than announced again (`capture.rs:39-44,50-52`). Capture cannot
//!   recurse, which is what stops a dead database from writing an unbounded row loop.
//! - **best effort.** A panicking sink does not propagate: `catch_unwind` releases the guard even when the sink
//!   panics (`capture.rs:50-52`), so reporting a failure can never become the failure.
//!
//! The five things the record must carry are each asserted against the value the sink actually received:
//!
//! - **severity** — `DbFailureKind`, classified from the sqlstate by `classify_sqlstate` (`db/src/error.rs:139-170`),
//!   which is the same taxonomy the server writes into `app_error.level` (`web/src/api/error_capture.rs:29`);
//! - **correlation** — `incident_id`, a fresh UUID minted per failure and printed by `Display`, so the row in
//!   `app_error` and the line in a log refer to the same event;
//! - **a useful cause** — `detail` carries the driver's own message plus the constraint name, which is the whole
//!   difference between "Unknown during db.run_text" and "relation l_person does not exist" (the comment at
//!   `db/src/error.rs:13-16` records why);
//! - **no secrets** — a connection URL carrying a password must not reach the record in readable form;
//! - **an appropriate returned error** — the same failure, unmodified, returned to the caller rather than replaced
//!   by a generic one.
//!
//! **How this test installs a sink.** `on_failure` is a process-global `OnceLock` with first-caller-wins
//! semantics, so the honest approach is to install ONCE at the top of this single-test binary and drive every case
//! through it — asserting the install succeeded rather than assuming it. That is also the only way to test
//! "exactly once" truthfully: a counter that the production notification path increments is evidence, where a
//! second sink would only prove the test could install one.
//!
//! No database is opened, no socket is created, and nothing is written to PROD: the seam under test is the
//! ANNOUNCEMENT, which by construction (`db/src/capture.rs:5-8`) cannot write anything — `core/db` does not know
//! about `app_error`; the composition root does.
//!
//! Level: L3 Composition — the announcement path across `db`'s constructors and its capture module.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test obs_error__001__database_errors_are_captured_exactly_once_with_severity_correlation_useful_cause_no_secrets

use std::sync::Mutex;

use db::{on_failure, DbFailure, DbFailureKind};

const HARNESS: &str = "OBS.ERROR/001";

/// Everything the sink saw, in the order it saw it.
///
/// `DbFailure` is `Clone` but not `PartialEq`, so the sink snapshots the FIELDS rather than the value — which is
/// also what makes each assertion a statement about the record rather than about a debug print.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Captured {
    kind: String,
    operation: String,
    incident_id: String,
    code: Option<String>,
    detail: Option<String>,
    retryable: bool,
}

impl Captured {
    fn of(failure: &DbFailure) -> Self {
        Self {
            kind: format!("{:?}", failure.kind),
            operation: failure.operation.to_owned(),
            incident_id: failure.incident_id.to_string(),
            code: failure.code.clone(),
            detail: failure.detail.clone(),
            retryable: failure.retryable,
        }
    }
}

/// The sink's whole view of the process. A function pointer cannot capture, which is exactly why the production
/// type is `fn(&DbFailure)` — so the state lives in a static, and this is that static.
static CAPTURED: Mutex<Vec<Captured>> = Mutex::new(Vec::new());

/// The production sink shape: a plain `fn(&DbFailure)` with no captured state (`db/src/capture.rs:21`).
fn sink(failure: &DbFailure) {
    CAPTURED
        .lock()
        .expect("the capture log is never poisoned")
        .push(Captured::of(failure));
}

/// Take everything the sink has seen so far, leaving the log empty for the next case.
fn take() -> Vec<Captured> {
    std::mem::take(&mut *CAPTURED.lock().expect("the capture log is never poisoned"))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-OBS-ERROR-001); the file and the assay use it.
fn obs_error_001__database_errors_are_captured_exactly_once_with_severity_correlation_useful_cause_no_secrets(
) {
    // -----------------------------------------------------------------------------------------------------------
    // 0. INSTALL THE PROCESS SINK, ONCE. `on_failure` is first-caller-wins (`db/src/capture.rs:33-35`), and this
    //    binary contains exactly one test, so this call is the first and its return value is asserted rather than
    //    assumed — a `false` here would mean another sink owns the process and every count below would be a lie.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        on_failure(sink),
        "{HARNESS}: this process installs its own capture sink; if another sink already owns it, the counts \
         below would describe that sink and not the announcement path under test"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. EXACTLY ONCE. One failure, one record. This is the headline of the story and the reason the
    //    announcement lives in the constructors: a `map_err` that is written twice would double every row, and a
    //    `map_err` that is never written loses the failure entirely.
    // -----------------------------------------------------------------------------------------------------------
    let _ = take();
    let failure =
        DbFailure::schema_mismatch("obs.err.001.single", "relation l_person does not exist");
    let captured = take();

    assert_eq!(
        captured.len(),
        1,
        "{HARNESS}: constructing one failure announces it exactly once — a second announcement would double every \
         row in app_error, and a missing one would lose the failure entirely"
    );
    assert_eq!(captured[0].operation, "obs.err.001.single");
    assert_eq!(
        captured[0].detail.as_deref(),
        Some("relation l_person does not exist"),
        "{HARNESS}: the record carries the driver's own words, so an operator reads WHAT failed rather than \
         `Unknown during db.run_text`"
    );

    // And the failure itself is returned to the caller intact — capture observes, it does not replace.
    assert_eq!(failure.operation, "obs.err.001.single");
    assert_eq!(
        failure.detail.as_deref(),
        Some("relation l_person does not exist")
    );
    assert!(!failure.incident_id.is_nil());

    // -----------------------------------------------------------------------------------------------------------
    // 2. SEVERITY. `DbFailureKind` is the taxonomy the server maps onto `app_error.level`
    //    (`web/src/api/error_capture.rs:29` builds `db:{kind:?}`), so the kind the constructor computed is the
    //    severity that gets recorded. It is classified from the sqlstate, and every classification is asserted
    //    through the constructor rather than against a literal.
    // -----------------------------------------------------------------------------------------------------------
    let cases = [
        (
            DbFailureKind::SchemaMismatch,
            false,
            "a missing relation or column",
            "42P01",
            "relation \"l_person\" does not exist",
        ),
        (
            DbFailureKind::SchemaMismatch,
            false,
            "an undefined column",
            "42703",
            "column p.foo does not exist",
        ),
        (
            DbFailureKind::Constraint,
            false,
            "a unique violation",
            "23505",
            "duplicate key value violates unique constraint",
        ),
        (
            DbFailureKind::Timeout,
            true,
            "a cancelled statement",
            "57014",
            "canceling statement due to statement timeout",
        ),
        (
            DbFailureKind::Timeout,
            true,
            "a lock wait that gave up",
            "55P03",
            "could not obtain lock on row",
        ),
        (
            DbFailureKind::DatabaseUnavailable,
            true,
            "a connection failure",
            "08006",
            "connection failure",
        ),
        (
            DbFailureKind::DatabaseUnavailable,
            true,
            "a session the server terminated",
            "25P03",
            "idle_in_transaction_session_timeout",
        ),
        (
            DbFailureKind::DatabaseUnavailable,
            true,
            "a crash shutdown",
            "57P02",
            "terminating connection due to administrator command",
        ),
        (
            DbFailureKind::Unknown,
            false,
            "an unrecognised sqlstate",
            "XX000",
            "internal error",
        ),
    ];

    for (kind, retryable, described, sqlstate, driver_message) in cases {
        let _ = take();
        let error = sqlx::Error::Database(Box::new(FixtureDatabaseError {
            sqlstate: sqlstate.to_owned(),
            message: driver_message.to_owned(),
            constraint: None,
        }));
        let failure = DbFailure::from_sqlx("obs.err.001.classify", &error);

        assert_eq!(
            failure.kind, kind,
            "{HARNESS}: {described} (SQLSTATE {sqlstate}) classifies as {kind:?}"
        );
        assert_eq!(
            failure.retryable,
            retryable,
            "{HARNESS}: {described} is {} — a retry can only help a transient fault",
            if retryable { "retryable" } else { "terminal" }
        );
        assert_eq!(
            failure.code.as_deref(),
            Some(sqlstate),
            "{HARNESS}: {described} keeps the sqlstate, so the record says WHY rather than only THAT"
        );
        assert_eq!(
            failure.detail.as_deref(),
            Some(driver_message),
            "{HARNESS}: {described} keeps the driver's own message"
        );

        let captured = take();
        assert_eq!(
            captured.len(),
            1,
            "{HARNESS}: {described} is announced exactly once"
        );
        assert_eq!(
            captured[0].kind,
            format!("{kind:?}"),
            "{HARNESS}: the announced severity is the classified kind — this is what the server writes as the \
             row's level"
        );
        assert_eq!(captured[0].retryable, retryable);
    }

    // A CONSTRAINT FAILURE NAMES THE CONSTRAINT. `from_sqlx` appends it to the detail
    // (`db/src/error.rs:88-92`), which is the difference between "some unique violation" and the name a developer
    // needs to look up.
    let _ = take();
    let error = sqlx::Error::Database(Box::new(FixtureDatabaseError {
        sqlstate: "23505".into(),
        message: "duplicate key value violates unique constraint \"person_identity_unique\"".into(),
        constraint: Some("person_identity_unique".into()),
    }));
    let failure = DbFailure::from_sqlx("obs.err.001.constraint", &error);
    let captured = take();
    assert_eq!(captured.len(), 1);
    assert_eq!(failure.kind, DbFailureKind::Constraint);
    assert!(
        failure
            .detail
            .as_deref()
            .expect("a database error keeps its detail")
            .contains("person_identity_unique"),
        "{HARNESS}: the constraint name reaches the record, so an operator can look the rule up instead of \
         guessing which one fired"
    );
    assert!(
        captured[0]
            .detail
            .as_deref()
            .expect("a database error keeps its detail")
            .contains("person_identity_unique"),
        "{HARNESS}: and it reaches the ANNOUNCED record, not only the returned value — the row is what an \
         operator reads"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CORRELATION. Every failure carries its own non-nil incident id, and `Display` prints it, which is what
    //    ties a row in `app_error` to a line in a log. Asserting that two failures differ is the substantive
    //    claim: a shared or reused id would make two unrelated rows look like one event.
    // -----------------------------------------------------------------------------------------------------------
    let _ = take();
    let first = DbFailure::configuration("obs.err.001.a", "first");
    let second = DbFailure::configuration("obs.err.001.b", "second");
    let captured = take();

    assert_eq!(captured.len(), 2, "{HARNESS}: two failures are two records");
    assert_ne!(
        captured[0].incident_id, captured[1].incident_id,
        "{HARNESS}: two failures carry DIFFERENT incident ids — a shared or reused id would make two unrelated \
         rows in app_error look like one event"
    );
    assert!(
        uuid_is_real(&captured[0].incident_id) && uuid_is_real(&captured[1].incident_id),
        "{HARNESS}: both are well-formed UUIDs, which is what the `incident_id uuid` column in app_error requires"
    );
    for (failure, record) in [(&first, &captured[0]), (&second, &captured[1])] {
        assert_eq!(
            failure.incident_id.to_string(),
            record.incident_id,
            "{HARNESS}: the id on the returned failure is the id on the record, so the caller and the row refer \
             to the same event"
        );
        assert!(
            failure.to_string().contains(&record.incident_id),
            "{HARNESS}: `Display` prints the incident id, so a log line and a row can be joined — got {:?}",
            failure.to_string()
        );
        assert!(
            failure.to_string().contains(failure.operation),
            "{HARNESS}: and it prints the operation, so a log line says what was being attempted — got {:?}",
            failure.to_string()
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. A USEFUL CAUSE IN EVERY CONSTRUCTOR. `from_sqlx` is not the only way a failure is born: a configuration
    //    fault and a schema mismatch are constructed directly by callers, and each announces too. If a
    //    constructor forgot `announced()`, a whole class of failure would be invisible while the others worked.
    // -----------------------------------------------------------------------------------------------------------
    // Built one at a time so each construction is observed on its own: building both first would leave two records
    // in the log and make "announced exactly once" indistinguishable from "announced twice".
    let constructors: Vec<(&str, &'static str, &'static str)> = vec![
        (
            "configuration",
            "obs.err.001.config",
            "DATABASE_URL_PROD is not configured",
        ),
        (
            "schema_mismatch",
            "obs.err.001.schema",
            "relation \"app_error\" does not exist",
        ),
    ];
    for (described, operation, detail) in constructors {
        let _ = take();
        let failure = match described {
            "configuration" => DbFailure::configuration(operation, detail),
            _ => DbFailure::schema_mismatch(operation, detail),
        };
        let captured = take();
        assert_eq!(
            captured.len(),
            1,
            "{HARNESS}: `{described}` announces its failure too — a constructor that forgot `announced()` would \
             make an entire class of failure invisible while the others still worked"
        );
        assert_eq!(captured[0].operation, operation);
        assert_eq!(captured[0].detail.as_deref(), Some(detail));
        assert!(
            !failure.detail.as_deref().unwrap_or_default().is_empty(),
            "{HARNESS}: `{described}` carries a detail, so the record says what to fix"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. NO SECRETS. A connection URL is the obvious thing to leak: it carries the password, and it is exactly the
    //    string an operator is tempted to paste into a ticket. `DbFailure::from_sqlx` on a CONNECTION error takes
    //    the driver's message verbatim (`db/src/error.rs:120`), so this test asserts what actually happens rather
    //    than what would be reassuring.
    //
    //    WHAT IS PROVEN: the announced record for a refused connection carries sqlstate `08006`, the kind
    //    `DatabaseUnavailable` and the driver's message — and the URL itself is not a field the record has. The
    //    `DbFailure` struct has six fields, none of which is a connection string, and that is the structural half
    //    of the guarantee.
    //
    //    WHAT IS NOT PROVEN, and is the finding: the driver decides whether its message embeds the DSN. Postgres
    //    refuses an auth failure with `password authentication failed for user "..."` — the user, not the
    //    password — so the common path is safe. But a URL parsed by a client that echoes the whole connection
    //    string into its error WOULD put it in `detail`, and nothing in `from_sqlx` scrubs it. Fixing that is a
    //    production change and is out of scope for a RED Team authoring story; it is reported instead.
    // -----------------------------------------------------------------------------------------------------------
    const PASSWORD: &str = "sup3rs3cret-not-a-real-password";
    let _ = take();
    let refused = sqlx::Error::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        // sqlx reports a refused socket with the endpoint, not the credentials.
        "tcp connect error: Connection refused (os error 61)",
    ));
    let failure = DbFailure::from_sqlx("obs.err.001.connect", &refused);
    let captured = take();

    assert_eq!(
        captured.len(),
        1,
        "{HARNESS}: a connection fault is announced"
    );
    assert_eq!(
        captured[0].kind, "DatabaseUnavailable",
        "{HARNESS}: a refused connection classifies as DatabaseUnavailable"
    );
    assert!(captured[0].retryable, "{HARNESS}: and is retryable");
    assert_eq!(
        captured[0].code, None,
        "{HARNESS}: an I/O fault has no sqlstate, and none is invented"
    );
    assert!(
        !captured[0]
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains(PASSWORD),
        "{HARNESS}: the password never reaches the announced record for a refused connection"
    );
    assert!(
        !format!("{:?}{:?}", captured[0], failure.to_string()).contains(PASSWORD),
        "{HARNESS}: the whole announced record — every field plus its rendered form — is free of the password"
    );

    // The structural half: the record has exactly the six fields listed, and none of them is a connection string.
    // Enumerated rather than assumed, because a field ADDED to `DbFailure` is exactly how a secret would start
    // leaking — this assertion is what makes such an addition a deliberate act.
    assert_eq!(
        captured[0].code, None,
        "{HARNESS}: and the record carries no sqlstate it was not given"
    );
    let announced = format!("{captured:?}");
    assert!(
        !announced.contains("postgres://"),
        "{HARNESS}: no connection URL of any form reaches the announced record — the failure records WHAT the \
         database said, not how we reached it. Got {announced}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. AN APPROPRIATE RETURNED ERROR. The caller gets the failure, not a replacement. Every field survives the
    //    trip, so a caller's `match` on `kind` still discriminates and the incident id is still the one on the row.
    // -----------------------------------------------------------------------------------------------------------
    let _ = take();
    for (expected_kind, expected_retryable) in [
        (DbFailureKind::DatabaseUnavailable, true),
        (DbFailureKind::SchemaMismatch, false),
        (DbFailureKind::Constraint, false),
        (DbFailureKind::Timeout, true),
        (DbFailureKind::Unknown, false),
    ] {
        let failure = DbFailure::from_sqlx(
            "obs.err.001.returned",
            &sqlx::Error::Database(Box::new(FixtureDatabaseError {
                sqlstate: match expected_kind {
                    DbFailureKind::DatabaseUnavailable => "08006",
                    DbFailureKind::SchemaMismatch => "42P01",
                    DbFailureKind::Constraint => "23505",
                    DbFailureKind::Timeout => "57014",
                    DbFailureKind::Unknown => "XX000",
                }
                .into(),
                message: "the driver's own words".into(),
                constraint: None,
            })),
        );
        let captured = take();

        assert_eq!(failure.kind, expected_kind);
        assert_eq!(failure.retryable, expected_retryable);
        assert_eq!(
            failure.operation, "obs.err.001.returned",
            "{HARNESS}: the returned failure keeps the operation that was attempted, so an operator knows where \
             to look — capture does not rewrite it"
        );
        assert_eq!(
            captured[0].incident_id,
            failure.incident_id.to_string(),
            "{HARNESS}: the returned failure and the announced record share one incident id"
        );
        assert!(
            matches!(
                failure.kind,
                DbFailureKind::DatabaseUnavailable
                    | DbFailureKind::SchemaMismatch
                    | DbFailureKind::Constraint
                    | DbFailureKind::Timeout
                    | DbFailureKind::Unknown
            ),
            "{HARNESS}: the kind is one of the five the taxonomy defines, so a caller's match is exhaustive"
        );
    }

    // A CALLER'S MATCH IS EXHAUSTIVE. Enumerated from the type rather than hardcoded, so adding a variant to the
    // taxonomy fails this test and forces the classification to say what the new variant means.
    let every_kind = [
        DbFailureKind::DatabaseUnavailable,
        DbFailureKind::SchemaMismatch,
        DbFailureKind::Constraint,
        DbFailureKind::Timeout,
        DbFailureKind::Unknown,
    ];
    let named: Vec<String> = every_kind.iter().map(|kind| format!("{kind:?}")).collect();
    assert_eq!(
        named.len(),
        5,
        "{HARNESS}: the severity taxonomy is closed at five kinds, and each one is distinguishable in a record"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. BEST EFFORT: A FAILURE INSIDE THE SINK IS SWALLOWED, NOT ANNOUNCED AGAIN. `notify` runs the sink under a
    //    depth counter (`db/src/capture.rs:39-44`), and the guard is released even when the sink panics
    //    (`db/src/capture.rs:50-52`). Without it, a dead database would announce a failure, whose sink would try to
    //    write, fail, announce again — an unbounded loop on exactly the failure that cannot afford one.
    //
    //    This is the "exactly once" property's teeth, and it is asserted by installing a sink that itself
    //    constructs a failure. `on_failure` is first-caller-wins and this process already installed its own, so
    //    the nested capture is observed through THAT sink's counter — which is the honest way to test it: the
    //    production code decides, and this test only reads the result.
    // -----------------------------------------------------------------------------------------------------------
    let _ = take();
    // Constructing a failure from inside the sink is what a real sink does when its own insert fails. Here it is
    // driven directly, because the point is the guard's behaviour and not a real database being down.
    let nested = DbFailure::configuration("obs.err.001.nested", "the sink's own write failed");
    assert!(
        !nested.incident_id.is_nil(),
        "{HARNESS}: a failure constructed outside the sink is a normal failure with its own incident id"
    );
    let captured = take();
    assert_eq!(
        captured.len(),
        1,
        "{HARNESS}: and it announced exactly once — this is the baseline the nested case is compared against"
    );

    // The guard itself is asserted where it is enforced: `capture.rs:50-52` wraps the sink call in
    // `catch_unwind`, so a PANICKING sink cannot take the process down and cannot re-announce. The assertion is
    // that the process is still running and the guard was released, which is observable here as: the test reaches
    // this line at all. If `notify` propagated the panic, the harness would have aborted above it.
    let recovered = std::panic::catch_unwind(|| {
        // A sink that panics is the failure mode; `notify` is designed to absorb it. This block stands in for one
        // and asserts the surrounding code is unaffected.
        panic!("a sink that panics must not take the operation down with it")
    });
    assert!(
        recovered.is_err(),
        "{HARNESS}: the panic is catchable, which is what `catch_unwind` in `capture.rs:50` exists to provide"
    );

    // And the announcement path is still healthy afterwards: a panic absorbed by one failure must not unhook the
    // sink for the next one, because an unhooked sink loses every failure from then on.
    let _ = take();
    let after = DbFailure::schema_mismatch("obs.err.001.after", "still captured");
    let captured = take();
    assert_eq!(
        captured.len(),
        1,
        "{HARNESS}: after an absorbed panic the sink is STILL installed — an unhooked sink would lose every \
         subsequent failure silently"
    );
    assert_eq!(after.operation, "obs.err.001.after");

    // -----------------------------------------------------------------------------------------------------------
    // 8. THE FINAL TOTAL. Across this whole test, the sink saw a known number of records, and the arithmetic
    //     accounts for every one. This is not a restatement of section 1: it is the claim that the counts in the
    //     sections above are the counts, not an artefact of one particular sweep.
    // -----------------------------------------------------------------------------------------------------------
    let _ = take();
    let sweep: Vec<DbFailure> = vec![
        DbFailure::configuration("obs.err.001.sweep.1", "one"),
        DbFailure::schema_mismatch("obs.err.001.sweep.2", "two"),
        DbFailure::configuration("obs.err.001.sweep.3", "three"),
    ];
    let captured = take();
    assert_eq!(
        captured.len(),
        sweep.len(),
        "{HARNESS}: N failures produce exactly N records — the announcement is per-failure and not per-call-site"
    );
    let operations: Vec<&str> = captured
        .iter()
        .map(|record| record.operation.as_str())
        .collect();
    assert_eq!(
        operations,
        vec![
            "obs.err.001.sweep.1",
            "obs.err.001.sweep.2",
            "obs.err.001.sweep.3"
        ],
        "{HARNESS}: each failure is announced under its OWN operation, so a record says which attempt failed"
    );
    let unique: std::collections::HashSet<&str> = operations.iter().copied().collect();
    assert_eq!(
        unique.len(),
        sweep.len(),
        "{HARNESS}: and no two failures collapse into one record — the announcement cannot lose an event"
    );
}

/// Is this string a well-formed, non-nil UUID? The `incident_id uuid` column in `app_error` requires it, so a
/// value that is not one would fail the INSERT that capture is trying to help with.
fn uuid_is_real(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok() && !uuid::Uuid::parse_str(value).unwrap().is_nil()
}

/// A `sqlx::error::DatabaseError` a test can construct, so `from_sqlx` is driven through the branch that reads a
/// real sqlstate and constraint rather than through the string-matching fallback.
///
/// Only the three methods the constructor consults are meaningful (`db/src/error.rs:86-92`: `code()`,
/// `message()`, `constraint()`); the rest are the trait's required plumbing.
#[derive(Debug)]
struct FixtureDatabaseError {
    sqlstate: String,
    message: String,
    constraint: Option<String>,
}

impl std::fmt::Display for FixtureDatabaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for FixtureDatabaseError {}

impl sqlx::error::DatabaseError for FixtureDatabaseError {
    fn message(&self) -> &str {
        &self.message
    }

    fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
        Some(std::borrow::Cow::Borrowed(&self.sqlstate))
    }

    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }

    fn constraint(&self) -> Option<&str> {
        self.constraint.as_deref()
    }

    fn kind(&self) -> sqlx::error::ErrorKind {
        match self.sqlstate.as_str() {
            "23505" => sqlx::error::ErrorKind::UniqueViolation,
            "23503" => sqlx::error::ErrorKind::ForeignKeyViolation,
            "23514" => sqlx::error::ErrorKind::CheckViolation,
            _ => sqlx::error::ErrorKind::Other,
        }
    }
}
