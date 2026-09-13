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
