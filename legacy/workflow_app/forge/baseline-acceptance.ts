/**
 * BASELINE ACCEPTANCE — a story whose proof already passes has nothing to prove.
 *
 * Found by the difficulty ladder on 2026-09-13: rung 2's frozen proof was already green at
 * the base commit, so the Smith shipped an unrelated one-line change and QA passed it. The
 * proof measured "does the acceptance hold", not "did this story do anything", and it held
 * before the story began. The story was unfalsifiable.
 *
 * This is the same principle the Architect already applies to CODE ("already-landed: the
 * exact assertion exists on baseRef, so no Builder change is legal") — applied to the PROOF,
 * which is the artifact the run is actually judged by. If every frozen command exits 0 on the
 * base commit, the acceptance is satisfied before any work, so no candidate can be proof of
 * work and the honest outcome is a HOLD that names the reason.
 *
 * Deliberately NOT a veto on partially-green acceptance: a story whose proofs partly pass has
 * real work left, and only the fully-satisfied case is unfalsifiable.
 *
 * Pure: the caller supplies the exit codes.
 */
export type BaselineCommandResult = {
  readonly command: string
  readonly exitCode: number | null
  /** True when the runner could not execute it (missing binary, timeout) — never a pass. */
  readonly unmeasurable?: boolean
}

export type BaselineAcceptanceVerdict =
  | { satisfiedAtBase: false; total: number; passed: number; reason: null }
  | { satisfiedAtBase: true; total: number; passed: number; reason: string }

export function assessBaselineAcceptance(
  results: readonly BaselineCommandResult[],
): BaselineAcceptanceVerdict {
  const total = results.length
  const passed = results.filter((r) => !r.unmeasurable && r.exitCode === 0).length
  // No commands is not "satisfied": it is unverifiable, which is a different problem.
  if (total === 0) return { satisfiedAtBase: false, total, passed, reason: null }
  if (passed !== total) return { satisfiedAtBase: false, total, passed, reason: null }

  const evidence = results.map((r) => r.command).join(' ; ')
  return {
    satisfiedAtBase: true,
    total,
    passed,
    reason:
      `BASELINE ACCEPTANCE: every frozen proof already exits 0 at the base commit ` +
      `(${passed}/${total}), so the story's acceptance holds BEFORE any work and no candidate ` +
      `can demonstrate it. A story must be falsifiable: fix the acceptance so it fails at base, ` +
      `or close the story as already-satisfied. Proofs: ${evidence}`,
  }
}
