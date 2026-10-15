// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::test_support::failure_hold_reason;

/// A human GATE (held, no reason) is a finished turn and settles `Done`; a hold that carries a reason is the
/// engine refusing to go on after a failure and must settle `Error`, or its run is closed `Complete` at 100%.
#[test]
fn only_a_hold_with_a_reason_is_a_failure_hold() {
    assert_eq!(failure_hold_reason(true, None), None, "a human gate");
    assert_eq!(
        failure_hold_reason(
            true,
            Some("Forge role smith failed for task t: refused".into())
        ),
        Some("Forge role smith failed for task t: refused".into())
    );
    assert_eq!(
        failure_hold_reason(true, Some("  ".into())),
        None,
        "a blank reason says nothing"
    );
    assert_eq!(
        failure_hold_reason(false, Some("no ready task; active: smith=Reserved".into())),
        None,
        "an exhausted run is not a hold; the settlement pair already refuses its Done"
    );
}
