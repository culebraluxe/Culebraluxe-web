# Phase 03: Story Verdict and Local Reproduction

This phase answers "is my Story bad?" by reproducing locally what Smith should have done for 002 (inequality) and 003 (boolean). It uses the landed 001 (equality, 473 lines) as the exemplar, checks the packets' acceptance criteria, and attempts to author the missing canonical test files in a scratch branch to prove they would PASS against the current production boundary in `middle/workflow/src/expr.rs`.

## Tasks

- [ ] Analyze the sibling that landed green as the template:
  - Re-read `tests/tests/wf_decision__001__equality.rs` structure: imports (EngineHarness, TestClock, obj), variables() fixture with multiple JSON types, condition() helper, refusal() helper, transition(), decision_definition() with two arms and no otherwise, start_with_level(), resting_node(), single test fn `wf_decision_001__equality`
  - Read packets `docs/agent/packets/TST-WF-DECISION-002.md` and `TST-WF-DECISION-003.md` for canonical file names, test fn names, scope rows: CANONICAL TEST FILE, TAXONOMY WF.DECISION L0 Pure HARNESS WorkflowHarness
  - Document comparison in `Working/Phase-03-exemplar-analysis.txt`: what 002 must prove (inequality is exact complement of equality, negation via !equal at expr.rs:22, coercion table) and what 003 must prove (boolean literal vocabulary true/false at expr.rs:62-63, boolean outcome routing, refusal table, non-vacuity)

- [ ] Locally author the missing canonical tests in scratch to prove story validity:
  - Create `Working/Phase-03-local-repro/wf_decision__002__inequality.rs` as a draft reproducing the inequality contract:
    - Use same harness as 001: EngineHarness::new(TestClock::at_unix_millis(1_700_000_000_000)), obj fixture with approved, rejected, status, count, flag
    - Prove: inequality true exactly when equality false (complement table for == vs !=), type-strict (count == "3" false thus != true), absence != null true, parser boundary agrees (is_supported_expression for !=), refusal table for garbage, decision boundary: engine routes on != arms
    - Include at least one negative/fault case per acceptance criterion 6
  - Create `Working/Phase-03-local-repro/wf_decision__003__boolean.rs` as draft for boolean:
    - Prove boolean literal parsing: "true" -> Bool(true), "false" -> Bool(false) via parse_literal and evaluate_condition
    - Prove routing on boolean: decision node with arms "approved == true" and "approved == false", test true selects true arm, false selects false arm
    - Prove non-vacuity: changing bool flips route, missing bool refuses, type-strict: bool vs string "true" false
    - Include refusal table for boolean-adjacent garbage
  - Both files must reference production boundary `workflow::expr::evaluate_condition` at `middle/workflow/src/expr.rs:10-28` — no second implementation

- [ ] Run local reproduction and record evidence:
  - Copy draft files temporarily to `tests/tests/wf_decision__002__inequality.rs` and `wf_decision__003__boolean.rs` only for local run (do not commit in this phase; these are evidence, not the engine's job)
  - Run `cargo test -p test-harness --test wf_decision__002__inequality -- --nocapture` and capture TEST_EXIT and output to `Working/Phase-03-002-result.txt` <!-- MAESTRO:MODEL tier="high" effort="high" reason="Designing a contract test that correctly proves inequality complement without reimplementing production logic is subtle. Wrong proof looks green but doesn't actually pin the invariant, so needs strongest model reasoning." -->
  - Run `cargo test -p test-harness --test wf_decision__003__boolean -- --nocapture` and capture to `Working/Phase-03-003-result.txt`
  - Run `cargo check --workspace --all-targets` to prove workspace stays green with both drafts present
  - If drafts pass, remove them from `tests/tests/` after capture to restore clean state (engine owns publishing, not this repro task); if they fail, keep output as evidence of production defect vs test-authoring defect per packet test-authoring policy

- [ ] Create story verdict report:
  - Write `docs/triage/Story-Verdict-2026-10-03.md` with front matter type: analysis, title: Story Verdict Local Repro, tags [wf-decision, test-authoring, local-repro], related [[TST-WF-DECISION-002-003-Triage-2026-10-03]] [[Engine-Plumbing-2026-10-03]] [[TST-WF-DECISION-001]] [[TST-WF-DECISION-002]] [[TST-WF-DECISION-003]]
  - Document: are packets healthy (same Assay as 001), does production boundary support != and bool (expr.rs lines 43-51 and 60-82), did local repro drafts PASS or FAIL and what that means per acceptance policy (correctly authored test exposing prod defect may still be committed)
  - Conclude with classification: Story Spec Fault vs Engine Fault vs Smith Turn Fault, with evidence paths to Working/ files and test outputs

- [ ] Final verification that local repro didn't leak:
  - Ensure `tests/tests/wf_decision__002__inequality.rs` and `wf_decision__003__boolean.rs` do NOT exist on disk after cleanup (engine owns publishing), unless this phase explicitly decided to keep them as evidence with clear note
  - Run `cargo check -p workflow -p test-harness --all-targets` clean
  - Confirm 001 baseline still passes: `cargo test -p test-harness --test wf_decision__001__equality -- --nocapture`
