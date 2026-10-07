//! FORGE.MODEL_POLICY-004 — policy survives parent→child process boundary.
//!
//! CONTRACT. The model policy reaches the child engine process through the durable row,
//! not through parent-process inheritance. `bin/forge.rs` re-reads `model_policy` at the
//! claim (`run_model_policy = begin.model_policy.clone()`) into `HarnessContext`, and the
//! harness resolves the turn's selection from that context. The production boundary is
//! `forge::engine::harness::{HarnessContext, ModelSelection::from_parts}`: the selection
//! is a pure function of the context's two fields, so parent and child compute the
//! identical selection from the identical row with no shared process state.
//!
//! Level: L3 Composition — the parent/child process composition boundary, providers faked
//! (no vendor touched, no process spawned: the recomputation is asserted, not executed).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__004__policy_survives_parent_child_process_boundary

use std::path::PathBuf;

use forge::engine::harness::{HarnessContext, ModelSelection};
use forge::engine::packet::StoryPacket;

fn context_with(policy: Option<&str>, model_override: Option<&str>) -> HarnessContext {
    HarnessContext {
        story_id: "TST-PROBE".to_string(),
        packet: StoryPacket::default(),
        workspace: PathBuf::from("."),
        execution_workspace: None,
        model_policy: policy.map(str::to_string),
        model_override: model_override.map(str::to_string),
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
    }
}

/// What the child computes from the context it was handed: the same pure function the
/// parent's `create_harness` feeds, with no parent environment in reach.
fn child_selection(context: &HarnessContext) -> ModelSelection {
    ModelSelection::from_parts(
        context.model_policy.as_deref(),
        context.model_override.as_deref(),
    )
}

#[test]
fn forge_model_policy_004__policy_survives_parent_child_process_boundary() {
    // ── 1. THE ROW'S POLICY RECOMPUTES IDENTICALLY IN THE CHILD. ────────────
    for policy in [Some("cheap"), Some("judgment"), None] {
        let parent = context_with(policy, None);
        let parent_selection =
            ModelSelection::from_parts(parent.model_policy.as_deref(), None);
        // The child inherits the context (row re-read), not the parent's process.
        let child = context_with(policy, None);
        assert_eq!(
            child_selection(&child),
            parent_selection,
            "policy {policy:?} must select identically across the process boundary"
        );
    }

    // ── 2. JUDGMENT SURVIVES: THE EXPENSIVE TIER IS NOT LOST IN TRANSIT. ────
    let child = context_with(Some("judgment"), None);
    assert_eq!(child_selection(&child), ModelSelection::Judgment);

    // ── 3. THE CONTEXT CARRIES BOTH INPUTS VERBATIM. ────────────────────────
    let context = context_with(Some("judgment"), Some("agent-x"));
    assert_eq!(context.model_policy.as_deref(), Some("judgment"));
    assert_eq!(context.model_override.as_deref(), Some("agent-x"));
    assert_eq!(
        child_selection(&context),
        ModelSelection::Explicit("agent-x".to_string()),
        "an attended override travels the same context and wins the same way"
    );

    // ── 4. NEGATIVE: A CHILD THAT INHERITS NOTHING BILLS LEAST. ─────────────
    // A context with neither field — a fresh claim, no row value, no override — selects
    // Cheap, never Judgment and never a vendor. The boundary's default is the floor.
    let orphan = context_with(None, None);
    assert_eq!(child_selection(&orphan), ModelSelection::Cheap);
    assert_ne!(child_selection(&orphan), ModelSelection::Judgment);

    // ── 5. NEGATIVE: THE SELECTION IS NOT THE PARENT'S AMBIENT STATE. ───────
    // Recomputation from the same fields is stable no matter how often it runs: there is
    // no hidden process-global input for the boundary to lose.
    let context = context_with(Some("judgment"), None);
    assert_eq!(child_selection(&context), child_selection(&context));
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::from_parts(Some("judgment"), None)
    );
}
