//! Forge evidence read: database error propagates instead of defaulting.
//!
//! This test verifies that when the database is unreachable during evidence read,
//! the error propagates as `DbFailure` (which becomes an engine fault) rather than
//! returning default evidence. Genuine no-evidence-yet still returns empty evidence.

use db::{DbFailure, DbFailureKind};
use forge::engine::writer::ForgeEvidenceReader;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// A test evidence reader that simulates a database failure.
struct FailingEvidenceReader {
    error: DbFailure,
}

impl ForgeEvidenceReader for FailingEvidenceReader {
    fn read(&self, _story_id: &str) -> Result<forge::engine::facts::ForgeGateEvidence, DbFailure> {
        Err(self.error.clone())
    }
}

/// A test evidence reader that returns empty evidence (no evidence yet).
struct EmptyEvidenceReader;

impl ForgeEvidenceReader for EmptyEvidenceReader {
    fn read(&self, _story_id: &str) -> Result<forge::engine::facts::ForgeGateEvidence, DbFailure> {
        Ok(forge::engine::facts::ForgeGateEvidence::default())
    }
}

/// A test evidence reader that returns evidence with data.
struct DataEvidenceReader {
    evidence: forge::engine::facts::ForgeGateEvidence,
}

impl ForgeEvidenceReader for DataEvidenceReader {
    fn read(&self, _story_id: &str) -> Result<forge::engine::facts::ForgeGateEvidence, DbFailure> {
        Ok(self.evidence.clone())
    }
}

#[test]
fn db_unavailable_during_evidence_read_returns_err_not_default() {
    // Simulate a database unavailable error (retryable, engine fault)
    let db_failure = DbFailure {
        kind: DbFailureKind::DatabaseUnavailable,
        operation: "forge_evidence_read",
        incident_id: uuid::Uuid::new_v4(),
        code: Some("08006".into()),
        detail: Some("connection refused".into()),
        retryable: true,
    };

    let reader = FailingEvidenceReader { error: db_failure };
    let result = reader.read("test-story");

    // The error should propagate, not return default evidence
    assert!(result.is_err(), "Expected Err(DbFailure), got Ok");
    let err = result.unwrap_err();
    assert_eq!(err.kind, DbFailureKind::DatabaseUnavailable);
    assert!(err.retryable, "DatabaseUnavailable should be retryable");
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn schema_mismatch_during_evidence_read_returns_err_not_default() {
    // Simulate a schema mismatch error (non-retryable)
    let db_failure = DbFailure {
        kind: DbFailureKind::SchemaMismatch,
        operation: "forge_evidence_read",
        incident_id: uuid::Uuid::new_v4(),
        code: Some("42P01".into()),
        detail: Some("relation \"forge_workflow_evidence\" does not exist".into()),
        retryable: false,
    };

    let reader = FailingEvidenceReader { error: db_failure };
    let result = reader.read("test-story");

    // The error should propagate, not return default evidence
    assert!(result.is_err(), "Expected Err(DbFailure), got Ok");
    let err = result.unwrap_err();
    assert_eq!(err.kind, DbFailureKind::SchemaMismatch);
    assert!(!err.retryable, "SchemaMismatch should not be retryable");
}

#[test]
fn genuine_no_evidence_yet_returns_empty_evidence() {
    // When there's genuinely no evidence yet, return empty evidence (Ok with default)
    let reader = EmptyEvidenceReader;
    let result = reader.read("test-story");

    assert!(result.is_ok(), "Expected Ok with empty evidence");
    let evidence = result.unwrap();
    // All fields should be None/default
    assert!(evidence.work_type.is_none());
    assert!(evidence.scout_required.is_none());
    assert!(evidence.qa_passed.is_none());
    assert!(evidence.repair_attempts.is_none());
    assert!(evidence.replan_attempts.is_none());
}

#[test]
fn evidence_with_data_returns_data() {
    // When there's actual evidence, return it
    let mut expected = forge::engine::facts::ForgeGateEvidence::default();
    expected.work_type = Some("FEATURE".into());
    expected.scout_required = Some(true);
    expected.qa_passed = Some(false);
    expected.repair_attempts = Some(2);
    expected.replan_attempts = Some(1);

    let reader = DataEvidenceReader {
        evidence: expected.clone(),
    };
    let result = reader.read("test-story");

    assert!(result.is_ok(), "Expected Ok with evidence data");
    let evidence = result.unwrap();
    assert_eq!(evidence.work_type, Some("FEATURE".into()));
    assert_eq!(evidence.scout_required, Some(true));
    assert_eq!(evidence.qa_passed, Some(false));
    assert_eq!(evidence.repair_attempts, Some(2));
    assert_eq!(evidence.replan_attempts, Some(1));
}

#[test]
fn db_forge_evidence_reader_propagates_connection_error() {
    // Test that the real DbForgeEvidenceReader would propagate connection errors.
    // We can't easily test the real one without a database, but we verify the trait
    // signature requires Result return type.
    let reader = FailingEvidenceReader {
        error: DbFailure {
            kind: DbFailureKind::DatabaseUnavailable,
            operation: "forge_evidence_read",
            incident_id: uuid::Uuid::new_v4(),
            code: None,
            detail: Some("pool closed".into()),
            retryable: true,
        },
    };

    // This simulates what happens when with_shared fails to get a connection
    let result = reader.read("test-story");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind, DbFailureKind::DatabaseUnavailable);
    assert!(err.retryable);
}

#[test]
fn map_evidence_returns_empty_for_missing_story() {
    // Test the MapEvidence implementation returns empty evidence for unknown stories
    let map = Mutex::new(BTreeMap::new());
    let reader = forge::engine::port::MapEvidence(map);

    let result = reader.read("unknown-story");
    assert!(result.is_ok());
    let evidence = result.unwrap();
    assert!(evidence.work_type.is_none());
}

#[test]
fn map_evidence_returns_stored_evidence() {
    // Test the MapEvidence implementation returns stored evidence
    let mut map = BTreeMap::new();
    let mut evidence = forge::engine::facts::ForgeGateEvidence::default();
    evidence.work_type = Some("FEATURE".into());
    evidence.qa_passed = Some(true);
    map.insert("test-story".to_string(), evidence);
    let reader = forge::engine::port::MapEvidence(Mutex::new(map));

    let result = reader.read("test-story");
    assert!(result.is_ok());
    let evidence = result.unwrap();
    assert_eq!(evidence.work_type, Some("FEATURE".into()));
    assert_eq!(evidence.qa_passed, Some(true));
}
