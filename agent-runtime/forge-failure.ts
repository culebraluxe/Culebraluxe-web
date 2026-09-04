// Forge V6 — typed failure vocabulary.
//
// Failure meaning is machine data. Human notes may describe a failure, but
// orchestration must never infer recovery behavior from prose such as "failed"
// or from a generic Hold status.

export type ForgeFailureCode =
  | 'PRECHECK_FAILED'
  | 'EXECUTION_CONTRACT_FAILED'
  | 'SMITH_RUNTIME_INTERRUPTED'
  | 'SMITH_RESULT_FAILED'
  | 'NO_CANDIDATE'
  | 'MISSING_ASSAY_PLAN'
  | 'ASSAY_TEST_FAILED'
  | 'ASSAY_POLICY_FAILED'
  | 'ASSAY_RUNTIME_INTERRUPTED'
  | 'CANDIDATE_MISMATCH'
  | 'PUBLISH_CONFLICT'
  | 'DEPENDENCY_BLOCKED'
  | 'HUMAN_DECISION_REQUIRED'

export type ForgeFailure = {
  code: ForgeFailureCode
  detail: string
  humanRequired: boolean
}

export function forgeFailure(
  code: ForgeFailureCode,
  detail: string,
  humanRequired = true,
): ForgeFailure {
  return { code, detail, humanRequired }
}
