//! The harness's own contract, exercised from outside the crate.
//!
//! These are the self-tests the foundation story requires: they prove the deterministic clock and id helpers are
//! actually deterministic, and they prove the PRODUCTION database guard refuses execution *before* any connection is
//! attempted. Everything here runs in `cargo test -p test-harness` with no database, no network and no environment.

use test_harness::database::{guard_target, resolve_test_target, HarnessDbError};
use test_harness::{DeterministicIds, FixtureFactory, TestClock, TestLevel};
use db::DbTarget;

#[test]
fn the_clock_is_deterministic_and_moves_only_when_asked() {
    // Two clocks at the same instant produce the same sequence, and neither moves on its own.
    let left = TestClock::at_unix_millis(1_700_000_000_000);
    let right = TestClock::at_unix_millis(1_700_000_000_000);
    assert_eq!(left.now_millis(), right.now_millis());
    assert_eq!(left.now_millis(), left.now_millis());

    let mut left_values = Vec::new();
    let mut right_values = Vec::new();
    for _ in 0..5 {
        left_values.push(left.advance_millis(250));
        right_values.push(right.advance_millis(250));
    }
    assert_eq!(left_values, right_values, "the same moves yield the same instants");
}

#[test]
fn the_id_stream_is_deterministic() {
    let left = DeterministicIds::new(1234);
    let right = DeterministicIds::new(1234);
    for _ in 0..16 {
        assert_eq!(left.next_uuid_string(), right.next_uuid_string());
        assert_eq!(left.next_u64(), right.next_u64());
        assert_eq!(left.id("row"), right.id("row"));
    }

    // A different seed diverges, so the determinism is a seed, not a constant.
    let other = DeterministicIds::new(1235);
    assert_ne!(DeterministicIds::new(1).next_uuid_string(), other.next_uuid_string());
}

#[test]
fn fixtures_are_deterministic_for_a_seed() {
    let left = FixtureFactory::new(9);
    let right = FixtureFactory::new(9);
    assert_eq!(left.namespace(), right.namespace());
    assert_eq!(left.uuid(), right.uuid());
    assert_eq!(left.email("guest"), right.email("guest"));
    assert_eq!(left.now_millis(), right.now_millis());
}

#[test]
fn the_production_database_guard_refuses_execution() {
    // 1. The pure guard: PROD is refused, DEV is allowed.
    let refusal = guard_target(DbTarget::Prod).expect_err("PROD must be refused");
    assert!(matches!(refusal, HarnessDbError::ProductionRefused(_)));

    // 2. The declaration the guard sits on: a production environment is refused, whichever variable says so.
    assert!(resolve_test_target(Some("production"), Some("dev")).is_err());
    assert!(resolve_test_target(None, Some("production")).is_err());
    assert!(resolve_test_target(None, Some("prod")).is_err());
    assert!(resolve_test_target(None, None).is_err(), "silence is refused, not defaulted");

    // 3. And a dev/test declaration resolves to DEV, so the guard is not simply refusing everything.
    assert_eq!(resolve_test_target(None, Some("test")).unwrap(), DbTarget::Dev);
    assert_eq!(
        resolve_test_target(Some("preview"), None).unwrap(),
        DbTarget::Dev
    );
}

#[test]
fn the_harness_supports_all_five_levels() {
    assert_eq!(TestLevel::ALL.len(), 5);
    assert_eq!(TestLevel::L0Pure.code(), "L0");
    assert_eq!(TestLevel::L4Adversarial.code(), "L4");
    assert!(TestLevel::L2Persistence.requires_database());
    assert!(!TestLevel::L0Pure.requires_database());
}
