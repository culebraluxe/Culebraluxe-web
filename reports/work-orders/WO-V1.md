# WO-V1 — persist UNPROVEN as a tri-state

- Work order: WO-V1 (Batch 1, order 2 of 3)
- Branch: `wo/V1` from integration base `7900e3daa6aa75a4ac58aafc707c7b34a1fc03d3`
- Spec source named by assignment: `Phase-02-Wave1-Parallel-Agent-Work-Orders.md`
  (not present in the base tree or the lane — see §1)
- Status: DONE, gates green. WO-207 (Wave 2) is unblocked by this landing.

## 1. HEAD verdict confirmation (baseline deviation)

`reports/baseline.md` does not exist at the base SHA, on `origin/main`, or in the lane, so
there is no D-2 verdict to cite. The current verdict at HEAD was confirmed by pinning the
exact chain in the base tree instead:

- `forge/src/engine/assay.rs:398-406` — the `!acceptance_mapped` arm writes
  `evidence.qa_passed = Some(false)` with `verdict: AssayVerdict::Unproven`: UNPROVEN is
  written as `qa_passed=false`. (Same `Some(false)` at assay.rs:369,379,391,407,415,432,454,463
  covers the other non-pass arms; only the Unproven arm changes here.)
- `forge/src/qa_consistency.rs:62` — `normalize_qa_verdict(Some(false))` → `Some(Fail)`.
- `forge/src/qa_consistency.rs:82` — `expected_verdict_for_run_status(Some("Failed"))` → `Some(Fail)`.
- `forge/src/qa_consistency.rs:118-121` — equal → `Agree { Failed, FAIL }`.
- `forge/src/qa_consistency.rs:141` — renders `agree (run Failed / verdict FAIL)`.
- `cli/src/forge/doctor.rs:356-357` feeds the durable boolean straight into that chain.

So at HEAD an unmeasured run reports `agree (Failed/FAIL)`: the bug. The boolean column
itself (`db/migrations/109…: qa_passed boolean`, nullable; `db/src/forge_doctor.rs:165`
reads `Option<bool>`) already has three states — the writer just never used the third.

## 2. Changes (only Owns files: `forge/src/roles/qa.rs`, `forge/src/qa_consistency.rs`)

Write side (`qa.rs`):

- New `apply_unproven_tristate(&mut evidence, &verdict)`: on `AssayVerdict::Unproven` sets
  `evidence.qa_passed = None` (no verdict recorded — never `false`). Called in
  `run_rust_contract_qa` and `read_assay_measurement`, AFTER `dispose_failure` (which reads
  the boolean to set ESCALATE) and BEFORE persist/return. Artifact token stays UNPROVEN,
  rejection and disposition survive; only the boolean stops claiming an unmeasured failure.
- Downstream audit (all outside Owns, all read-only): `facts.rs:182` (`!= Some(true)`) and
  `facts.rs:370` (`== Some(true)`) treat None like false already; `facts.rs:326` routes only
  `Some(false)`, but UNPROVEN stories are held via `deliverable_rejection` first, and the UI
  flags (`repair_el`/`replan_el`) are false either way since Fail+ESCALATE routes to neither
  Smith nor Architect; `phase.rs:141` QaVerdict-deliverable is `PhaseDeliverableKind::None`
  for measurement nodes (`AssayHooks::deliverable_kind`), so no new missing-deliverable.

Read side (`qa_consistency.rs`):

- `QaVerdict` gains `Unproven` (`as_str` → `"UNPROVEN"`); `normalize_qa_verdict_token`
  maps `"UNPROVEN"` (case-insensitive, like the other two).
- `QaConsistency` gains `Unproven { run_status, detail }`: `state()` returns `"unknown"`
  (doctor-safe — counted unknown, never agree, never a disagreement), while
  `render_qa_consistency_line` returns `UNPROVEN — {detail}`. `check_*` short-circuits an
  Unproven verdict to it for ANY run status.

## 3. Red → green (7 new tests, all in Owns files)

- Compile-red first run: `no variant Unproven`, `cannot find apply_unproven_tristate`.
- Runtime-red probe: with the two `apply_unproven_tristate` call sites briefly reverted,
  `unproven_is_persisted_as_no_verdict_not_as_failed` FAILED (`Some(false)` vs `None`);
  re-applied → green.
- Green: `cargo test -p forge --lib` → **391 passed, 0 failed** (7 new); base tree runs
  384 (391 − 7), so nothing else moved.
- `cargo clippy -p forge --all-targets` → **0 errors**; `cargo check -p cli` green
  (dependents compile; the doctor matches on `state()` strings, unaffected).
- Flake note: one mid-work full-suite run showed 3 transient failures in unrelated
  env-sensitive tests (incl. `engine::config::heartbeat_override_clamped`); two
  consecutive re-runs fully green, base tree green, all re-runs green. Pre-existing
  parallel-env flakiness, not this change.

New tests: `qa_consistency::an_unmeasured_run_reports_unproven_never_failed_fail`,
`a_measured_fail_on_a_failed_run_still_agrees` (overcorrection guard),
`unproven_is_a_token_spelling_of_its_own`; `qa::unproven_is_persisted_as_no_verdict_not_as_failed`
(end to end through the contract rig), `measured_fail_is_still_persisted_as_failed_with_repair`,
`measured_pass_is_still_persisted_as_passed`, `unproven_tristate_clears_only_the_boolean_not_the_record`.

## 4. Commits

- Work commit (code + tests): `5d362a82d8fd8ae75364023bd40e03e2b849b97e`
- Report commit (this file): names the work commit; branch tip at push is the receipt commit.

## 5. Raw gate output (tail)

```text
test result: ok. 391 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 8.27s
```

```text
clippy: 0 errors (grep -cE "^error" → 0)
```

## 6. Batch 28 re-verification (Meta Muse2, lane-muse-2)

- Prior branch state (`origin/wo/V1` tip `202822a28`) carried the work but the report
  file had been removed from the tip ("archived separately", archive not found on
  `origin/main` or in the lane). This commit restores the report (§1–§5 verbatim from
  `c70b14963`) with this addendum.
- Re-verified on this branch (ff from `origin/wo/V1`, no code changes):
  `cargo test -p forge --lib` → **391 passed, 0 failed**;
  `cargo clippy -p forge --all-targets` → **0 errors**.
- Independent red probe by this worker: temporarily reverted the two
  `apply_unproven_tristate` call sites → `roles::qa::` tests **FAILED (6 passed,
  1 failed)**, failing test `unproven_is_persisted_as_no_verdict_not_as_failed`
  (`Some(false)` vs `None`); restored → full suite **391 passed, 0 failed**.
  Red before, green after, confirmed in this batch.
- Work commit unchanged: `5d362a82d8fd8ae75364023bd40e03e2b849b97e` (parent is the
  integration base `7900e3daa`). Net branch diff vs base is only the two Owns files.
  WO-207 (Wave 2) remains unblocked by this landing.
