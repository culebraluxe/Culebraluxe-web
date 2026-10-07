//! FORGE.LAUNCH_INTENT-004 — HOLD prevents Smith execution.
//!
//! CONTRACT. A `HOLD` bench intent (migration 167 `launch_intent`) stops Smith work. The
//! production boundary is twofold, and this test holds both:
//!
//! 1. `forge::engine::serial_doors::serial_launch_door` — the serial Smith lane (`smith`,
//!    `repair_smith`, `fast_smith`, `fast_repair_smith`) may not launch without an accepted
//!    Lead assignment. The door's HOLD names the reason: Smith does not choose its own scope.
//! 2. `forge::engine::role_slice::bench_intent_errors` — the cap: a `HOLD` intent refuses
//!    every decision that is not `HOLD`, including `SMITH`.
//!
//! Level: L1 Component — pure production functions, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__004__hold_prevents_smith_execution

use forge::engine::role_slice::bench_intent_errors;
use forge::engine::serial_doors::{serial_launch_door, NO_ASSIGNMENT_REASON};

#[test]
fn forge_launch_intent_004__hold_prevents_smith_execution() {
    // ── 1. THE DOOR: SMITH WITHOUT AN ASSIGNMENT IS HELD. ──────────────────
    for node in [
        "smith",
        "repair_smith",
        "fast_smith",
        "fast_repair_smith",
    ] {
        let hold = serial_launch_door(node, false);
        let reason = hold.unwrap_or_else(|| panic!("{node} without an assignment must HOLD"));
        assert!(
            reason.contains("HOLD") && reason.contains(NO_ASSIGNMENT_REASON),
            "{node} must HOLD naming why Smith waits: {reason}"
        );
    }

    // ── 2. THE DOOR OPENS ONLY ON AN ACCEPTED ASSIGNMENT. ───────────────────
    for node in [
        "smith",
        "repair_smith",
        "fast_smith",
        "fast_repair_smith",
    ] {
        assert_eq!(
            serial_launch_door(node, true),
            None,
            "{node} with an accepted assignment must launch"
        );
    }

    // ── 3. NEGATIVE: THE DOOR IS A SMITH DOOR, NOT A GLOBAL ONE. ────────────
    assert_eq!(
        serial_launch_door("lead_pre", false),
        None,
        "a non-Smith node must never be held by the serial Smith door"
    );
    assert_eq!(
        serial_launch_door("assay", false),
        None,
        "QA must never be held by the serial Smith door"
    );

    // ── 4. THE CAP: HOLD REFUSES THE SMITH DECISION. ────────────────────────
    let errors = bench_intent_errors(Some("HOLD"), Some("SMITH"));
    assert!(
        !errors.is_empty(),
        "a HOLD bench intent must refuse the SMITH decision"
    );
    assert_eq!(
        errors,
        vec!["Bench intent is HOLD: the Lead decided SMITH".to_string()],
        "the refusal must name the cap and the decision exactly"
    );

    // ── 5. NEGATIVE: HOLD REFUSES EVERYTHING BUT HOLD. ──────────────────────
    for decision in ["SOLO", "SPLIT", "ASSAY"] {
        assert!(
            !bench_intent_errors(Some("HOLD"), Some(decision)).is_empty(),
            "HOLD must refuse decision {decision}, or the SMITH refusal would be special-casing"
        );
    }

    // ── 6. CONTROL: HOLD AUTHORIZES HOLD, AND NOTHING MORE. ─────────────────
    assert!(
        bench_intent_errors(Some("HOLD"), Some("HOLD")).is_empty(),
        "HOLD must authorize HOLD itself"
    );
}
