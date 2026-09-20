//! Port of forge-architect-directive.ts.

pub fn build_architect_directive(base_ref: &str, frozen_proofs: &[String]) -> String {
    let proofs: Vec<_> = frozen_proofs.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let base = if base_ref.is_empty() { "<missing — HOLD if you cannot name the SHA you inspected>" } else { base_ref };
    format!(
        "ARCHITECT: survey the frozen story against the pinned baseRef. Do not implement. Do not choose SOLO/SMITH/SPLIT.\n\
baseRef={base}\n\
For each finding emit: preconditions, scope (existing repo paths on baseRef, ≤3), postconditions, classes, risks, required, hint.\n\
hint is topology only: SAME_UNIT | SPLIT_CHILD | FOLLOW_UP_STORY | NOTE | HOLD. A required HOLD must name a concrete risk.\n\
Adjacent work is required:false. An active decision is an input to the contract.\n\
Frozen proofs (do not invent commands): {}\n\
End with exactly ONE un-fenced JSON line beginning FORGE_ARCHITECT_HANDOFF:.",
        if proofs.is_empty() { "(none declared on the story)".into() } else { proofs.join(" | ") }
    )
}
