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

export type QaVerdict = 'PASS' | 'FAIL' | 'INCOMPLETE'

export type QaReport = {
  version: 1
  /** Candidate Smith produced. */
  evaluatedSha: string
  verifiedSha: string | null
  verdict: QaVerdict
  commands: CommandResult[]
  staticGate: StaticSlice | null
  blockers: string[]
  /** Present only when a model is invited AFTER a FAIL to classify, never to verdict. */
  failureClass?: string | null
}

export type AssayPlan = {
  candidateSha: string
  /** Frozen story/chunk proofs. Empty plan is INCOMPLETE, never PASS. */
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
