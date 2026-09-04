// Forge V6/V6.1 — typed failure vocabulary.
// Failure meaning is machine data. Human notes may describe a failure, but
// orchestration must never infer recovery behavior from prose.

export type ForgeFailureCode =
  | 'PRECHECK_FAILED'
  | 'EXECUTION_CONTRACT_FAILED'
  | 'SCOUT_HANDOFF_INVALID'
  | 'ARCHITECT_CONTRACT_INVALID'
  | 'LEAD_ARCHITECTURE_CHALLENGE'
  | 'LEAD_DECISION_MISSING'
  | 'LEAD_SPLIT_REQUIRES_MULTIWORKER'
  | 'LEAD_SPLIT_INVALID'
  | 'LEAD_INTEGRATION_FAILED'
  | 'LEAD_RUNTIME_INTERRUPTED'
  | 'SMITH_RUNTIME_INTERRUPTED'
  | 'SMITH_RESULT_FAILED'
  | 'SMITH_SPLIT_FAILED'
  | 'NO_CANDIDATE'
  | 'QA_DECISION_MISSING'
  | 'QA_REVIEW_FAILED'
  | 'QA_RUNTIME_INTERRUPTED'
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
