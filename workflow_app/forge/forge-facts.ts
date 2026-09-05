import type { ApplicationFacts } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Item 2 — Forge decision-gate facts projection.
//
// Every <decision> in FORGE_SDLC-v1.xml evaluates conditions against facts the
// engine refreshes from the application (ApplicationPort.readFacts -> engine
// _refreshFacts -> merged into instance variables). This module is the single
// projector: given the structured execution evidence recorded by the Forge
// layer when a task/step completes (and any static story facts), it returns the
// normalized gate-fact map.
//
// Policy: booleans default FALSE (a gate holds until positive evidence exists —
// conservative, never advances a story past a gate it has not actually passed).
// Router/enum facts (workType, leadDecision, failureClass, resumeTarget,
// researchDisposition) have no safe default and must be provided by evidence;
// when absent they are omitted so a missing decision route fails closed.
//
// Pure, DB-free, unit-testable.
// ---------------------------------------------------------------------------

export type ForgeGateEvidence = {
  /** classify_work */
  workType?: 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION'
  /** research_disposition */
  researchDisposition?: 'IMPLEMENT' | 'ARCHIVE' | 'HOLD'
  /** feature_scout_needed / bug diagnosis */
  scoutRequired?: boolean
  rootCauseKnown?: boolean
  diagnosisBlocked?: boolean
  architectureSuspect?: boolean
  /** execution_shape */
  leadDecision?: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'
  splitCount?: number
  /** qa_policy */
  qaReviewRequired?: boolean
  /** qa_review_result */
  qaReviewPassed?: boolean
  /** qa_result */
  qaPassed?: boolean
  /** failure_route */
  failureClass?:
    | 'CODE_DEFECT'
    | 'TEST_DEFECT'
    | 'ARCHITECTURE_GAP'
    | 'REQUIREMENTS_GAP'
    | 'UNKNOWN_CAUSE'
    | 'ENVIRONMENT'
    | 'MIGRATION'
    | 'PUBLISH_CONFLICT'
    | 'DEPLOYMENT'
    | 'PRODUCTION_SMOKE'
    | 'HOLD'
  /** devops_resume_router */
  failedReleaseStage?:
    | 'PUBLISH'
    | 'DEV_MIGRATION'
    | 'PROD_MIGRATION'
    | 'DERIVED_REFRESH'
    | 'DEPLOY'
    | 'SMOKE'
  /** publish_result */
  publishSucceeded?: boolean
  /** migration_required */
  migrationRequired?: boolean
  /** dev/prod migration verification */
  devMigrationVerified?: boolean
  prodMigrationVerified?: boolean
  /** derived refresh */
  derivedRefreshRequired?: boolean
  derivedRefreshVerified?: boolean
  /** deploy */
  deploymentRequired?: boolean
  deploymentSucceeded?: boolean
  /** production_result */
  productionVerified?: boolean
  /** hold_resolution */
  resumeTarget?:
    | 'SCOUT'
    | 'DIAGNOSE'
    | 'ARCHITECT'
    | 'LEAD'
    | 'SMITH'
    | 'QA'
    | 'DEV_OPS'
    | 'PUBLISH'
    | 'DEPLOY'
    | 'SMOKE'
    | 'CANCEL'
}

/** Booleans default to false; omitted when absent. */
export function projectForgeGateFacts(evidence: ForgeGateEvidence): ApplicationFacts {
  const facts: ApplicationFacts = {}

  // classify / disposition / shape / resume are enum routers: emit only when
  // evidence exists so a missing value fails the gate closed instead of routing
  // on a fabricated default.
  const enumFacts: Record<string, string | undefined> = {
    workType: evidence.workType,
    researchDisposition: evidence.researchDisposition,
    leadDecision: evidence.leadDecision,
    failureClass: evidence.failureClass,
    failedReleaseStage: evidence.failedReleaseStage,
    resumeTarget: evidence.resumeTarget,
  }
  for (const [key, value] of Object.entries(enumFacts)) {
    if (value !== undefined) facts[key] = value
  }
  if (evidence.splitCount !== undefined) facts.splitCount = evidence.splitCount

  // Booleans: default false (gate holds until positive evidence exists).
  const boolFacts: Record<string, boolean | undefined> = {
    scoutRequired: evidence.scoutRequired,
    rootCauseKnown: evidence.rootCauseKnown,
    diagnosisBlocked: evidence.diagnosisBlocked,
    architectureSuspect: evidence.architectureSuspect,
    qaReviewRequired: evidence.qaReviewRequired,
    qaReviewPassed: evidence.qaReviewPassed,
    qaPassed: evidence.qaPassed,
    publishSucceeded: evidence.publishSucceeded,
    migrationRequired: evidence.migrationRequired,
    devMigrationVerified: evidence.devMigrationVerified,
    prodMigrationVerified: evidence.prodMigrationVerified,
    derivedRefreshRequired: evidence.derivedRefreshRequired,
    derivedRefreshVerified: evidence.derivedRefreshVerified,
    deploymentRequired: evidence.deploymentRequired,
    deploymentSucceeded: evidence.deploymentSucceeded,
    productionVerified: evidence.productionVerified,
  }
  for (const [key, value] of Object.entries(boolFacts)) {
    facts[key] = value ?? false
  }
  return facts
}
