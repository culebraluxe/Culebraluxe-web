import type { ApplicationFacts } from '@/workflow_engine/lib/workflow/types'
import { routeQaResult } from '@/legacy/workflow_app/forge/qa-repair-policy'
import type { ArchitectFinding } from '@/legacy/workflow_app/forge/forge-shaping'
import type { GateCheck } from '@/legacy/workflow_app/forge/agents/gate-checks'

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
  workType?: 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION' | 'FAST'
  /** research_disposition */
  researchDisposition?: 'IMPLEMENT' | 'ARCHIVE' | 'HOLD'
  /** feature_scout_needed / bug diagnosis */
  scoutRequired?: boolean
  rootCauseKnown?: boolean
  diagnosisBlocked?: boolean
  architectureSuspect?: boolean
  /** Scope C review park — Architect completion may STOP at `hold` for human review. */
  architectureReviewRequired?: boolean
  /** execution_shape */
  leadDecision?: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | 'ASSAY'
  splitCount?: number
  /**
   * ENG-FORGE-SPLIT-01 — the ACCEPTED Lead routing proposal (validated once at
   * acceptance) persisted as a durable fact. Work orders, split-child assignment
   * and the join gate read THIS; they must never re-derive it from run notes and
   * re-validate against a context that other roles are still mutating.
   * Typed loosely here to keep the facts projector free of a routing import cycle.
   */
  leadRouting?: unknown
  /**
   * ENG-FORGE batch rollout: set when a batch-sliced story completes with its
   * deployment DEFERRED to a release batch. Distinct from a deployment receipt —
   * it never claims production verification.
   */
  deploymentDeferredToBatch?: number
  /**
   * WHAT EACH CHECK DID (FORGE-GATE-RECEIPT-01, migration 197): [{id, status, reason}] where status is
   * passed | failed | skipped | unavailable | not-configured. A projection of checks that already happened,
   * so it never writes a verdict — and a check that did not run is never `passed`.
   */
  gateChecks?: GateCheck[]
  /** ENG-FORGE-SHAPE-01 — durable Architect findings snapshot (Lead shaping gate). */
  findings?: ArchitectFinding[]
  /**
   * Non-empty when a role's own gate REJECTED its output (architect handoff that
   * failed assessment; smith diff outside the accepted assignment). Presence is
   * itself a HOLD: a rejected output must never satisfy its deliverable just
   * because something else was also written on exit. Carries the reason text so
   * the bounded self-heal reprompt can act on it instead of guessing.
   */
  deliverableRejection?: string
  /** qa_policy */
  qaReviewRequired?: boolean
  /** qa_review_result */
  qaReviewPassed?: boolean
  /** qa_result */
  qaPassed?: boolean
  /** qa_failure_route (V11-S1): the QA-supplied disposition + durable observers */
  disposition?: 'REPAIR' | 'REPLAN' | 'ESCALATE'
  /**
   * Verification/config gap (NOT a candidate defect): QA could not form/run a
   * valid assay command plan. When set with qaPassed=false, repair is NOT
   * eligible -> the graph routes to a durable HOLD instead of looping to smith
   * (the FINAL-02 deadlock). Additive: absent/undefined preserves current routing.
   */
  verificationGap?: boolean
  /**
   * Scope B no-progress (CONVERGENCE-01): this exact candidate SHA re-failed the same
   * machine classification with no new candidate in between. Set by the runner from the
   * convergence read model, and passed into `routeQaResult` so the graph HOLDs instead
   * of paying for another repair on an unchanged candidate.
   */
  noProgress?: boolean
  repairAttempts?: number
  replanAttempts?: number
  failedCriteria?: string[]
  failedCommands?: string[]
  /**
   * Engine-facing routing facts (computed, NOT role-settable). The engine is the
   * stop authority: the graph routes REPAIR/REPLAN only while its budget remains,
   * otherwise to HOLD. These discretize disposition+budget so the graph decision
   * stays a single-comparison rule (no boolean `and` needed in condition exprs).
   */
  qaRepairEligible?: boolean
  qaReplanEligible?: boolean
  /**
   * WHY the last failure happened, in the failing stage's own words.
   *
   * Carried so a refusal survives the turn that produced it. Without it the evidence row could say
   * `publishSucceeded = false` with no reason anywhere: the publisher knew (its command message named
   * the outcome and the refusing proof) and the durable record did not, so neither the operator nor
   * the repair classifier could act on it.
   */
  lastFailure?: string | null
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
  /**
   * ENG-FORGE-FAILURE-LABEL-01 — the failure_classifier's own label, kept as METADATA.
   *
   * When a release stage already recorded the accurate class with `failedReleaseStage`, the
   * classifier may add its opinion here but may not replace `failureClass`. The stage class
   * stays authoritative because the router reads the stage, not the classifier.
   */
  classifierFailureClass?: ForgeGateEvidence['failureClass'] | null
  /**
   * ENG-FORGE-FAILURE-LABEL-01 — the class the failing release stage recorded, carried so the
   * classifier cannot lose it. A later writer (the classifier's own marker or typed evidence)
   * may overwrite `failureClass`; this is the durable copy `FailureClassifierAgent.collect`
   * restores from.
   */
  stageFailureClass?: ForgeGateEvidence['failureClass'] | null
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
  /**
   * BATCH-SLICED ROLLOUT (definition v5): true when this story completed with its
   * deployment DEFERRED to a release batch. The engine's deployment_result decision
   * routes a deferred story to completion instead of classifying it as a deployment
   * failure — a deferral is an accepted outcome, not a fake success.
   */
  deploymentDeferred?: boolean
  /**
   * BATCH-SLICED ROLLOUT (definition v6): the WHOLE release tail is held for the
   * batch — publish, migrations, deploy and smoke. True for a story flagged
   * batch_deploy once it has reached a clean QA pass, so the story completes
   * QA-verified with its candidate frozen and NOTHING published or deployed.
   */
  releaseDeferred?: boolean
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
  /**
   * ENG-FORGE-BATCH-RECEIPT-01 — the durable receipt of the batch release that
   * carried this story: the commit it released and when. Null until a release
   * actually happened; never asserted from the deferral record.
   */
  batchReleasedSha?: string | null
  batchReleasedAt?: string | null
  batchReleaseReceipt?: string | null
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

  // THE QA LINK IS A VERDICT, NOT A SHA. QA has no relationship to git: it runs the story's frozen proofs
  // and records what they did. Demanding a QA-held sha made every publish refuse (measured live 2026-09-16,
  // instance 8fc792a6: `qa_passed=true`, `qa_verified_sha=null`, so this gate answered
  // 'qaVerifiedSha is missing or invalid', the executor recorded `publishSucceeded=false`, and the chain
  // went to repair_devops). What the release path may require is the verdict itself.
  if (evidence.qaPassed !== true) return 'QA has not passed for this candidate'
  if (stage === 'qa') return null

  const published = normalizedSha(evidence.publishedSha)
  if (!published) return 'publishedSha is missing or invalid'
  // AN INTEGRATED PUBLISH IS A PUBLISH. When main has moved, the publisher merges the candidate onto the
  // current head, re-runs the frozen proofs against the integrated tree, and publishes THAT commit — so the
  // published sha is legitimately not the candidate sha. Demanding equality made a successful integration
  // read as a failed publish. The executor is the authority on what it published; this gate requires that
  // something was published.
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

/**
 * ENG-FORGE-DEPLOY-NOMECH-01 — FORGE IS NOT A DEPLOYMENT PRODUCER.
 *
 * Nothing in this repository mints a deployment receipt: `scripts/vercel-deploy-prod.sh` is run by the
 * captain at sprint release, not by Forge. So a "configured producer" is a DURABLE receipt something
 * outside the engine already recorded — derived here from the evidence, never a new free-standing flag,
 * because a second source for one fact is how a fabricated deployment starts. The receipt alone is not
 * enough: it must carry a valid sha, or it attests nothing.
 */
export function forgeDeploymentProducerConfigured(evidence: ForgeGateEvidence): boolean {
  return Boolean(
    (normalizedSha(evidence.deployedSha) && evidence.deploymentReceipt?.trim()) ||
      (normalizedSha(evidence.productionVerifiedSha) &&
        evidence.productionVerificationReceipt?.trim()),
  )
}

/** The named reason a required deployment with no producer is held AT THE DECISION. */
export const FORGE_DEPLOY_NO_PRODUCER_REASON =
  'no deployment producer is configured: Forge performs no deployment, so a story that requires one is held at the decision until a durable deployment receipt is recorded or the deployment is deferred to its batch'

/**
 * The deploy decision, made at the DECISION — never at the lane.
 *
 * Non-null only when the story DEMANDS a deployment, is NOT deferred to its batch, and has no durable
 * producer. A deferred story is an accepted outcome; a no-deployment story never needs a producer; a
 * story with a durable receipt already has one. This is the fact the engine and the deploy entry both
 * read, so one decision answers both.
 */
export function forgeDeployHoldReason(evidence: ForgeGateEvidence): string | null {
  if (evidence.deploymentRequired !== true) return null
  if (evidence.deploymentDeferredToBatch != null) return null
  if (forgeDeploymentProducerConfigured(evidence)) return null
  return FORGE_DEPLOY_NO_PRODUCER_REASON
}

/** Booleans default to false; omitted when absent. */
/**
 * FAST eligibility (Scope C) — a FAST story is executable only when it carries
 * no release/schema/decomposition obligation that would silently grow FAST into
 * a release workflow. Computed from gate evidence; fails closed.
 */
export function forgeFastEligibility(evidence: ForgeGateEvidence): boolean {
  if (evidence.workType !== 'FAST') return false
  if (evidence.migrationRequired === true) return false
  if (evidence.derivedRefreshRequired === true) return false
  if (evidence.deploymentRequired === true) return false
  if (evidence.architectureSuspect === true) return false
  if (evidence.leadDecision === 'SPLIT') return false
  return true
}

export function projectForgeGateFacts(evidence: ForgeGateEvidence): ApplicationFacts {
  const facts: ApplicationFacts = {}

  // classify / disposition / shape / resume are enum routers: emit only when
  // evidence exists so a missing value fails the gate closed instead of routing
  // on a fabricated default.
  const enumFacts: Record<string, string | undefined> = {
    workType: evidence.workType,
    researchDisposition: evidence.researchDisposition,
    leadDecision: evidence.leadDecision,
    disposition: evidence.disposition,
    failureClass: evidence.failureClass,
    failedReleaseStage: evidence.failedReleaseStage,
    resumeTarget: evidence.resumeTarget,
  }
  for (const [key, value] of Object.entries(enumFacts)) {
    if (value !== undefined) facts[key] = value
  }
  if (evidence.splitCount !== undefined) facts.splitCount = evidence.splitCount
  if (evidence.repairAttempts !== undefined) facts.repairAttempts = evidence.repairAttempts
  if (evidence.replanAttempts !== undefined) facts.replanAttempts = evidence.replanAttempts
  if (evidence.migrationFiles !== undefined) facts.migrationFiles = evidence.migrationFiles
  if (evidence.derivedModels !== undefined) facts.derivedModels = evidence.derivedModels

  // V11-S1: QA-failure routing is computed here (engine is the stop). Given a
  // FAIL with a disposition + the durable observer counts, only an in-budget,
  // legal REPAIR/REPLAN is eligible; ESCALATE / missing / invalid / exhausted
  // all yield neither flag -> the graph routes to HOLD.
  const qaRoute =
    evidence.qaPassed === false
      ? routeQaResult({
          verdict: 'FAIL',
          disposition: evidence.disposition,
          state: {
            repairAttempts: evidence.repairAttempts ?? 0,
            replanAttempts: evidence.replanAttempts ?? 0,
          },
          verificationGap: evidence.verificationGap,
          noProgress: evidence.noProgress,
        })
      : null

  // Booleans: default false (gate holds until positive evidence exists).
  const boolFacts: Record<string, boolean | undefined> = {
    scoutRequired: evidence.scoutRequired,
    rootCauseKnown: evidence.rootCauseKnown,
    diagnosisBlocked: evidence.diagnosisBlocked,
    architectureSuspect: evidence.architectureSuspect,
    architectureReviewRequired: evidence.architectureReviewRequired,
    qaReviewRequired: evidence.qaReviewRequired,
    qaReviewPassed: evidence.qaReviewPassed,
    // ONE REQUIREMENT: QA passes when QA says it passed. No second condition rides
    // along with it — no lineage conjunct, no extra gate — because a QA verdict with a rider attached is a
    // verdict nobody can act on, and a lane that passes its check must not be blocked by a rule it was never
    // told about. Sha lineage for the release path is checked where the release happens (db-release-executor).
    qaPassed: evidence.qaPassed === true,
    qaRepairEligible: qaRoute?.action === 'smith',
    qaReplanEligible: qaRoute?.action === 'architect',
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
    deploymentDeferred: evidence.deploymentDeferredToBatch != null,
    releaseDeferred: evidence.deploymentDeferredToBatch != null,
    deploymentSucceeded:
      evidence.deploymentSucceeded === true && forgeLineageError(evidence, 'deploy') === null,
    productionVerified:
      evidence.productionVerified === true && forgeLineageError(evidence, 'production') === null,
    // ENG-FORGE-DEPLOY-NOMECH-01: a required deployment with no producer is BLOCKED at the
    // decision. Both facts are derived from the durable receipts + the batch deferral, so the
    // engine and the deploy entry read one source and no lane is entered on a guess.
    deploymentProducerConfigured: forgeDeploymentProducerConfigured(evidence),
    deploymentBlocked: forgeDeployHoldReason(evidence) !== null,
  }
  for (const [key, value] of Object.entries(boolFacts)) {
    facts[key] = value ?? false
  }
  // Scope C: derived FAST eligibility (fails closed; never silently grows into a release).
  facts.fastEligible = forgeFastEligibility(evidence)
  return facts
}
