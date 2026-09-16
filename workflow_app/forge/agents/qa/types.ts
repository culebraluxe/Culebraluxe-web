/**
 * Assay verdict types. `semgrepFindings` / `knipFindings` are optional on
 * purpose: they ride along for the record and never flip a verdict, so a
 * static slice that does not collect them is still valid input.
 */

export type CommandResult = {
  command: string
  exitCode: number
  passed: boolean
  excerpt: string
  /**
   * TRUE when the command could not be RUN at all — a spawn error (bad cwd, missing binary) or a timeout
   * kill — as opposed to running and reporting failure.
   *
   * These were the same thing until 2026-09-15, and it made QA look flaky and too strict: `spawnSync`
   * returns `status: null` for a command it never started, the runner coerced that to `exitCode: 1`, and
   * the adjudicator recorded `CMD_FAIL <command>` for a proof that passes when it can actually be run
   * (measured: the doctor's frozen proof exits 0 with 11/11 in the candidate worktree, while the QA lane
   * recorded it as failed). "We could not check" is not "we checked and it broke" — the module already
   * distinguishes those for an empty plan; it must do it per command too.
   */
  unmeasurable?: boolean
}

export type StaticSlice = {
  archRan: boolean
  archOk: boolean
  archErrors: string[]
  semgrepFindings?: string[]
  knipFindings?: string[]
}

/**
 * THE VERDICT IS "DID THE TESTS PASS" (Captain, 2026-09-16).
 *
 * Two states, because that is the whole question. There used to be a third, `INCOMPLETE`, for "we could not
 * run it" — and it existed to route a gap to a HOLD instead of sending repair after untested code. Repair
 * is no longer dispatched on a QA failure at all (fail once, stop), so the third state has no job left: a
 * command that could not run did not pass, and each command's own `unmeasurable` flag records WHY on the
 * row. QA advises nothing and promotes nothing; it reports what the tests did.
 */
export type QaVerdict = 'PASS' | 'FAIL'

export type QaReport = {
  version: 1
  verdict: QaVerdict
  commands: CommandResult[]
  staticGate: StaticSlice | null
  blockers: string[]
}

/**
 * What QA is given: the story's frozen proofs, and nothing else. No candidate, no SHA, no lineage — QA has
 * no relationship to git and no role in committing, promoting or advising on a release.
 */
export type AssayPlan = {
  /** Frozen story/chunk proofs. Empty means there is nothing to test. */
  commands: string[]
}

export const FAILURE_CLASSES = [
  'CODE_DEFECT',
  'TEST_DEFECT',
  'ARCHITECTURE_GAP',
  'REQUIREMENTS_GAP',
  'UNKNOWN_CAUSE',
  'ENVIRONMENT',
  'MIGRATION',
  'PUBLISH_CONFLICT',
  'DEPLOYMENT',
  'PRODUCTION_SMOKE',
  'HOLD',
] as const

export type FailureClass = (typeof FAILURE_CLASSES)[number]
