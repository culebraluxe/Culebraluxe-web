//! FORGE.QA — failure-class route budget enforcement (TST-FORGE-QA-011).
//!
//! CONTRACT. `budgeted_failure_class` is the ceiling on the failure-class router. When a failure class has exhausted
//! its budget, the function returns `Some("HOLD")` — the class is demoted to HOLD and the engine stops asking the
//! same worker to try again. Below the budget it returns `None` — the class is left alone for normal routing.
//!
//! The budget rules per class:
//! - `BadImplementation` and `WeakTest` → charged against `max_repair`
//! - `BadArchitecture` → charged against `max_replan`
//! - All other classes → charged against the sum of repair + replan attempts, capped at `max_repair`
//!
//! Classes with no repair owner (`Unknown`, `DependencyFailure`) always return `Some("HOLD")` regardless of budget,
//! because `route_failure` has no owner to route them to.
//!
//! Level: L3 Composition — exercises the production `budgeted_failure_class` function from `forge::engine::failure`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__011__failure_class_route_budget

use forge::engine::failure::{
    ForgeFailureClass, budgeted_failure_class,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_011__failure_class_route_budget() {
    // ── BadImplementation: charged against max_repair (3). ───────────────────────────────────────────

    // Below budget: attempts 0, 1, 2 → None (no demotion).
    for attempts in 0..3u32 {
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::BadImplementation, attempts, 0, 3, 2),
            None,
            "BadImplementation at repair={attempts} (max=3) must not be demoted"
        );
    }
    // At budget: attempts=3 → Some("HOLD").
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::BadImplementation, 3, 0, 3, 2),
        Some("HOLD"),
        "BadImplementation at repair=3 (max=3) must be demoted to HOLD"
    );

    // ── WeakTest: also charged against max_repair. ────────────────────────────────────────────────────

    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::WeakTest, 2, 0, 3, 2),
        None,
        "WeakTest at repair=2 (max=3) must not be demoted"
    );
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::WeakTest, 3, 0, 3, 2),
        Some("HOLD"),
        "WeakTest at repair=3 (max=3) must be demoted to HOLD"
    );

    // ── BadArchitecture: charged against max_replan (2). ─────────────────────────────────────────────

    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::BadArchitecture, 0, 1, 3, 2),
        None,
        "BadArchitecture at replan=1 (max=2) must not be demoted"
    );
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::BadArchitecture, 0, 2, 3, 2),
        Some("HOLD"),
        "BadArchitecture at replan=2 (max=2) must be demoted to HOLD"
    );

    // ── Classes without their own counter: charged against repair + replan sum, capped at max_repair. ──

    // EnvironmentFailure: repair=2, replan=1 → sum=3, max_repair=3 → HOLD (budget exhausted).
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::EnvironmentFailure, 2, 1, 3, 2),
        Some("HOLD"),
        "EnvironmentFailure at repair=2+replan=1=3 (max=3) must be demoted to HOLD"
    );
    // EnvironmentFailure: repair=1, replan=0 → sum=1, max_repair=3 → None (under budget, has owner).
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::EnvironmentFailure, 1, 0, 3, 2),
        None,
        "EnvironmentFailure at repair=1+replan=0=1 (max=3) must not be demoted"
    );

    // MissingContext: repair=2, replan=1 → sum=3 → HOLD.
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::MissingContext, 2, 1, 3, 2),
        Some("HOLD"),
        "MissingContext at repair=2+replan=1=3 (max=3) must be demoted to HOLD"
    );
    // MissingContext: repair=0, replan=0 → sum=0 → None.
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::MissingContext, 0, 0, 3, 2),
        None,
        "MissingContext at zero attempts must not be demoted"
    );

    // ── Classes with no repair owner: always Hold regardless of budget. ──────────────────────────────

    // Unknown has no owner in route_failure → always Some("HOLD").
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::Unknown, 0, 0, 3, 2),
        Some("HOLD"),
        "Unknown has no repair owner and must always be demoted to HOLD"
    );
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::Unknown, 1, 1, 3, 2),
        Some("HOLD"),
        "Unknown must always be demoted to HOLD regardless of budget"
    );

    // DependencyFailure has no owner → always Some("HOLD").
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::DependencyFailure, 0, 0, 3, 2),
        Some("HOLD"),
        "DependencyFailure has no repair owner and must always be demoted to HOLD"
    );

    // ── NEGATIVE CONTROL: at zero attempts, classes WITH an owner are never demoted. ─────────────────

    for class in [
        ForgeFailureClass::BadImplementation,
        ForgeFailureClass::WeakTest,
        ForgeFailureClass::BadArchitecture,
        ForgeFailureClass::MissingContext,
        ForgeFailureClass::BadToolContract,
        ForgeFailureClass::MissingGuardrail,
        ForgeFailureClass::EnvironmentFailure,
        ForgeFailureClass::DeploymentFailure,
    ] {
        assert_eq!(
            budgeted_failure_class(class, 0, 0, 3, 2),
            None,
            "{class:?} at zero attempts must never be demoted"
        );
    }
}
