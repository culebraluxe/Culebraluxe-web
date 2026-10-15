//! One pure helper the bin's own tests share: whether a hold carrying this reason is a failure hold. Moved
//! out of `main.rs` on 2026-10-14 to keep the bin root inside the ceiling — move-only apart from
//! `pub(super)`, so `main.rs` and `tests.rs` reach it at the same path they always did.

/// A human GATE ends a turn with no blocked reason and settles `Done` (the pair accepts `Done` over `Hold`). A hold
/// that carries a reason is the engine refusing to go on after a failure, and must settle `Error`, or its run is
/// closed `Complete` at 100%.
pub(super) fn failure_hold_reason(
    needs_human: bool,
    blocked_reason: Option<String>,
) -> Option<String> {
    blocked_reason.filter(|reason| needs_human && !reason.trim().is_empty())
}
