import type { ApplicationFacts } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 — Forge decision-gate facts projection.
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
  migrationFiles?: string[]
  devMigrationApplied?: boolean
  /** dev/prod migration verification */
  devMigrationVerified?: boolean
  prodMigrationApplied?: boolean
  prodMigrationVerified?: boolean
  /** derived refresh */
  derivedRefreshRequired?: boolean
  derivedModels?: string[]
  derivedRefreshSucceeded?: boolean
  derivedRefreshVerified?: boolean
  /** deploy */
  deploymentRequired?: boolean
  deploymentSucceeded?: boolean
  deploymentReceipt?: string | null
  /** production_result */
  productionVerified?: boolean
  productionVerificationReceipt?: string | null
  /** Immutable artifact identity chain. */
  candidateSha?: string | null
  qaVerifiedSha?: string | null
  publishedSha?: string | null
  deployedSha?: string | null
  productionVerifiedSha?: string | null
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

export type ForgeLineageStage = 'qa' | 'publish' | 'deploy' | 'production'

function normalizedSha(value: string | null | undefined): string | null {
  const sha = value?.trim().toLowerCase() ?? ''
  return /^[0-9a-f]{7,64}$/.test(sha) ? sha : null
}

/**
 * Return null only when the exact-candidate invariant is satisfied through the
 * requested stage. This is intentionally independent of the boolean gate:
 * `qaPassed=true` never substitutes for artifact identity.
 */
export function forgeLineageError(
  evidence: ForgeGateEvidence,
  stage: ForgeLineageStage,
): string | null {
  const candidate = normalizedSha(evidence.candidateSha)
  if (!candidate) return 'candidateSha is missing or invalid'

  const qa = normalizedSha(evidence.qaVerifiedSha)
  if (!qa) return 'qaVerifiedSha is missing or invalid'
  if (qa !== candidate) return `QA verified ${qa}, expected candidate ${candidate}`
  if (stage === 'qa') return null

  const published = normalizedSha(evidence.publishedSha)
  if (!published) return 'publishedSha is missing or invalid'
  if (published !== candidate) return `published ${published}, expected candidate ${candidate}`
  if (stage === 'publish') return null

  if (stage === 'deploy') {
    const deployed = normalizedSha(evidence.deployedSha)
    if (!deployed) return 'deployedSha is missing or invalid'
    if (deployed !== published) return `deployed ${deployed}, expected published ${published}`
    return null
  }

  const expectedProduction = evidence.deploymentRequired
    ? normalizedSha(evidence.deployedSha)
    : published
  if (!expectedProduction) return 'production artifact source SHA is missing or invalid'
  const verified = normalizedSha(evidence.productionVerifiedSha)
  if (!verified) return 'productionVerifiedSha is missing or invalid'
  if (verified !== expectedProduction) {
    return `production verified ${verified}, expected ${expectedProduction}`
  }
  return null
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
  if (evidence.migrationFiles !== undefined) facts.migrationFiles = evidence.migrationFiles
  if (evidence.derivedModels !== undefined) facts.derivedModels = evidence.derivedModels

  // Booleans: default false (gate holds until positive evidence exists).
  const boolFacts: Record<string, boolean | undefined> = {
    scoutRequired: evidence.scoutRequired,
    rootCauseKnown: evidence.rootCauseKnown,
    diagnosisBlocked: evidence.diagnosisBlocked,
    architectureSuspect: evidence.architectureSuspect,
    qaReviewRequired: evidence.qaReviewRequired,
    qaReviewPassed: evidence.qaReviewPassed,
    qaPassed: evidence.qaPassed === true && forgeLineageError(evidence, 'qa') === null,
    publishSucceeded:
      evidence.publishSucceeded === true && forgeLineageError(evidence, 'publish') === null,
    migrationRequired: evidence.migrationRequired,
    devMigrationApplied: evidence.devMigrationApplied,
    devMigrationVerified: evidence.devMigrationVerified,
    prodMigrationApplied: evidence.prodMigrationApplied,
    prodMigrationVerified: evidence.prodMigrationVerified,
    derivedRefreshRequired: evidence.derivedRefreshRequired,
    derivedRefreshSucceeded: evidence.derivedRefreshSucceeded,
    derivedRefreshVerified: evidence.derivedRefreshVerified,
    deploymentRequired: evidence.deploymentRequired,
    deploymentSucceeded:
      evidence.deploymentSucceeded === true && forgeLineageError(evidence, 'deploy') === null,
    productionVerified:
      evidence.productionVerified === true && forgeLineageError(evidence, 'production') === null,
  }
  for (const [key, value] of Object.entries(boolFacts)) {
    facts[key] = value ?? false
  }
  return facts
}
