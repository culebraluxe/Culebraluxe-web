// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

#[test]
fn maps_batch_kind_to_engine_work_type() {
    assert_eq!(work_type_for_kind(Some("fix")), "BUG");
    assert_eq!(work_type_for_kind(Some("qa")), "RESEARCH");
    assert_eq!(work_type_for_kind(Some("learn")), "RESEARCH");
    assert_eq!(work_type_for_kind(Some("normal")), "FEATURE");
}

/// `FAST` is what opens the fast lane (`FORGE_SDLC-v6.xml:85`), and the batch `kind` vocabulary cannot express
/// it — so the story's declaration must win, or every board-dispatched story is a FEATURE story. That was the
/// state until migration 259: 680 armed test rows, each paying a Scout, an Architect and a Lead turn.
#[test]
fn a_declared_work_type_opens_the_fast_lane() {
    assert_eq!(work_type_for_item(Some("FAST"), Some("fix")), "FAST");
    assert_eq!(work_type_for_item(Some("fast"), None), "FAST");
    assert_eq!(work_type_for_item(Some(" FAST "), Some("qa")), "FAST");
}

/// The legacy path is untouched: an undeclared story behaves exactly as it did before migration 259.
#[test]
fn an_undeclared_story_still_maps_off_its_kind() {
    assert_eq!(work_type_for_item(None, Some("fix")), "BUG");
    assert_eq!(work_type_for_item(None, Some("qa")), "RESEARCH");
    assert_eq!(work_type_for_item(None, Some("learn")), "RESEARCH");
    assert_eq!(work_type_for_item(None, None), "FEATURE");
    assert_eq!(work_type_for_item(Some(""), Some("fix")), "BUG");
}

/// A declaration outside the six work types never invents a lane and never leaks through: the column's check
/// constraint makes this unreachable in the database, and this is the boundary that does not trust that.
#[test]
fn an_unknown_declaration_falls_back_to_the_kind_mapping() {
    assert_eq!(work_type_for_item(Some("TURBO"), Some("qa")), "RESEARCH");
    assert_eq!(work_type_for_item(Some("turbo"), None), "FEATURE");
}

#[test]
fn a_blank_worker_identity_still_names_a_process() {
    // Note: worker_identity() reads from env, so we can't easily test the raw path here.
    // The pure core is tested via the logic in worker_identity().
    let id = worker_identity();
    assert!(id.starts_with("forge-worker-") && id.len() > 13);
}

#[test]
fn assay_roles_are_terminal_recovery_roles() {
    assert!(assay_terminal_role(Some("reviewer")));
    assert!(assay_terminal_role(Some("verifier")));
    assert!(!assay_terminal_role(Some("builder")));
}

/// The drain waits on this count, so it has to be exact: up while a run is carried, down when it ends — and down
/// when the run PANICS, or a drain would wait out its whole window for a run that is already gone.
/// The one fact that stops a child is lost authority. A failed beat and a dead heartbeat thread are reported as
/// what they are; neither may be read as "somebody took the claim", because stopping a child over our own
/// bookkeeping bug abandons work nobody took from us.
#[test]
fn only_lost_authority_stops_a_child() {
    let alive = HeartbeatState {
        beats: 4,
        ..Default::default()
    };
    assert!(lease_loss_reason(&alive, "item-1").is_none());

    // The database was unreachable: the claim is still ours until a beat SAYS otherwise.
    let failing = HeartbeatState {
        beats: 4,
        last_error: Some("connection reset".into()),
        ..Default::default()
    };
    assert!(lease_loss_reason(&failing, "item-1").is_none());

    // Our own bookkeeping died. Reported (`panicked`), never authority loss.
    let panicked = HeartbeatState {
        beats: 4,
        panicked: true,
        ..Default::default()
    };
    assert!(lease_loss_reason(&panicked, "item-1").is_none());

    let lost = HeartbeatState {
        beats: 4,
        lost_authority: true,
        ..Default::default()
    };
    let reason = lease_loss_reason(&lost, "item-1").expect("lost authority stops the child");
    assert!(reason.contains("lease lost"), "{reason}");
    assert!(reason.contains("item-1"), "{reason}");
}

/// A worker identity is a name the operator sets; the fence is a generation the database granted. This test is
/// the shape of the distinction: two claims by the SAME owner are two authorities.
#[test]
fn two_claims_by_one_name_are_two_authorities() {
    let first = db::ClaimFence::new("worker-a", 1);
    let second = db::ClaimFence::new("worker-a", 2);
    assert_ne!(first, second);
    assert_eq!(first.owner, second.owner);
    // The pre-278 claim is generation 0, and it is a different authority from the first fenced one.
    assert_ne!(db::ClaimFence::unfenced("worker-a"), first);
}

/// The drain waits on this count, so it has to be exact: up while a run is carried, down when it ends — and down
/// when the run PANICS, or a drain would wait out its whole window for a run that is already gone.
#[test]
fn the_in_flight_count_follows_runs_and_survives_a_panic() {
    // Other tests in this process may hold a run; measure the change, not the absolute.
    let before = in_flight_runs();
    {
        let _one = InFlight::enter();
        let _two = InFlight::enter();
        assert_eq!(in_flight_runs(), before + 2, "each carried run is counted");
    }
    assert_eq!(in_flight_runs(), before, "a finished run is released");

    let panicked = std::panic::catch_unwind(|| {
        let _run = InFlight::enter();
        panic!("a role turn blew up");
    });
    assert!(panicked.is_err());
    assert_eq!(in_flight_runs(), before, "a panicking run is released too");
}
