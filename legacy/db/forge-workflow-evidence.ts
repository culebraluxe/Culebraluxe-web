import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'
import type { BatchReleaseReceipt } from '@/legacy/workflow_app/forge/forge-release-receipt'
import type { ArchitectFinding } from '@/legacy/workflow_app/forge/forge-shaping'
import type { GateCheck } from '@/legacy/workflow_app/forge/agents/gate-checks'
import { classifyStoredQaDisposition } from '@/legacy/workflow_app/forge/qa-repair-policy'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('@/legacy/db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

type EvidenceRow = QueryRow & Record<string, unknown>

const value = <T>(row: EvidenceRow, key: string): T | undefined => {
  const current = row[key]
  return current === null || current === undefined ? undefined : (current as T)
}

/** Driver timestamps leave this boundary as ISO strings or null — never a Date. */
const isoOrNull = (current: unknown): string | null => {
  if (current === null || current === undefined) return null
  if (current instanceof Date) return current.toISOString()
  const parsed = Date.parse(String(current))
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString()
}

const stringArray = (row: EvidenceRow, key: string): string[] | undefined => {
  const current = row[key]
  if (current === null || current === undefined) return undefined
  const parsed = typeof current === 'string' ? JSON.parse(current) : current
  return Array.isArray(parsed) && parsed.every((item) => typeof item === 'string')
    ? parsed
    : undefined
}

const findingsArray = (row: EvidenceRow, key: string): ArchitectFinding[] | undefined => {
  const current = row[key]
  if (current === null || current === undefined) return undefined
  let parsed: unknown = current
  if (typeof parsed === 'string') {
    try {
      parsed = JSON.parse(parsed)
    } catch {
      return undefined
    }
  }
  if (!Array.isArray(parsed)) return undefined
  const out: ArchitectFinding[] = []
  for (const item of parsed) {
    if (!item || typeof item !== 'object') continue
    const f = item as Record<string, unknown>
    const id = String(f.id ?? '').trim()
    const summary = String(f.summary ?? '').trim()
    if (!id || !summary) continue
    const seams = Array.isArray(f.seams)
      ? f.seams.filter((s): s is string => typeof s === 'string').map((s) => s.trim()).filter(Boolean)
      : []
    const hint = String(f.hint ?? '').toUpperCase()
    const hintValue = (['SAME_UNIT', 'SPLIT_CHILD', 'FOLLOW_UP_STORY', 'NOTE', 'HOLD'] as const).find(
      (h) => h === hint,
    )
    out.push({
      id,
      summary,
      required: f.required === true,
      seams,
      hint: hintValue,
    })
  }
  return out
}

export function mapForgeWorkflowEvidence(row: EvidenceRow): ForgeGateEvidence {
  return {
    workType: value(row, 'work_type'),
    researchDisposition: value(row, 'research_disposition'),
    scoutRequired: value(row, 'scout_required'),
    rootCauseKnown: value(row, 'root_cause_known'),
    diagnosisBlocked: value(row, 'diagnosis_blocked'),
    architectureSuspect: value(row, 'architecture_suspect'),
    architectureReviewRequired: value(row, 'architecture_review_required'),
    leadDecision: value(row, 'lead_decision'),
    splitCount: value(row, 'split_count'),
    leadRouting: value(row, 'lead_routing'),
    deploymentDeferredToBatch: value(row, 'deployment_deferred_to_batch'),
    findings: findingsArray(row, 'findings'),
    qaReviewRequired: value(row, 'qa_review_required'),
    qaReviewPassed: value(row, 'qa_review_passed'),
    qaPassed: value(row, 'qa_passed'),
    failureClass: value(row, 'failure_class'),
    failedReleaseStage: value(row, 'failed_release_stage'),
    // The reason a failure happened, in the failing stage's own words. The column existed and was
    // never read or written, so a publish that refused recorded `publish_succeeded = false` with no
    // reason anywhere - the operator could see THAT it failed and never WHY.
    lastFailure: value(row, 'last_failure'),
    publishSucceeded: value(row, 'publish_succeeded'),
    migrationRequired: value(row, 'migration_required'),
    migrationFiles: stringArray(row, 'migration_files'),
    devMigrationApplied: value(row, 'dev_migration_applied'),
    devMigrationVerified: value(row, 'dev_migration_verified'),
    prodMigrationApplied: value(row, 'prod_migration_applied'),
    prodMigrationVerified: value(row, 'prod_migration_verified'),
    derivedRefreshRequired: value(row, 'derived_refresh_required'),
    derivedModels: stringArray(row, 'derived_models'),
    derivedRefreshSucceeded: value(row, 'derived_refresh_succeeded'),
    derivedRefreshVerified: value(row, 'derived_refresh_verified'),
    deploymentRequired: value(row, 'deployment_required'),
    deploymentSucceeded: value(row, 'deployment_succeeded'),
    deploymentReceipt: value(row, 'deployment_receipt'),
    productionVerified: value(row, 'production_verified'),
    productionVerificationReceipt: value(row, 'production_verification_receipt'),
    resumeTarget: value(row, 'resume_target'),
    candidateSha: value(row, 'candidate_sha'),
    qaVerifiedSha: value(row, 'qa_verified_sha'),
    publishedSha: value(row, 'published_sha'),
    deployedSha: value(row, 'deployed_sha'),
    productionVerifiedSha: value(row, 'production_verified_sha'),
    batchReleasedSha: value(row, 'batch_released_sha'),
    gateChecks: (row['gate_checks'] as GateCheck[] | null) ?? undefined,
    batchReleasedAt: isoOrNull(row['batch_released_at']),
    batchReleaseReceipt: value(row, 'batch_release_receipt'),
  }
}

/**
 * The merge payload plus the ONE explicit signal the release path needs.
 *
 * The failure markers (`failure_class`, `failed_release_stage`, `last_failure`) use
 * `coalesce`, so an omitted OR null value PRESERVES the stored marker — a failure not
 * yet followed by a success must keep its markers. `releaseFailureResolved: true` is
 * the explicit clear the release executor writes with the success that resolves a
 * stage, so the router cannot send a resolved story back to the stage that succeeded.
 * It follows the `lead_decision`/`split_count` precedent: an explicit write is
 * authoritative, every other write preserves what is known.
 */
export type ForgeEvidenceMerge = ForgeGateEvidence & {
  releaseFailureResolved?: boolean
  /**
   * ENG-FORGE-FENCE-CAN-FAIL-01 — the negative-control outcome for the story's declared fence.
   *
   * `ran` records whether the control executed at all; `killingAssertions` names the mapped
   * assertions that went red under it. A control that ran and killed none is UNPROVEN, never
   * proof. Omitted values coalesce, so a later write that knows nothing about the control
   * preserves what an earlier write recorded.
   */
  negativeControl?: {
    ran: boolean
    unmeasurable?: boolean
    killingAssertions: string[]
  }
}

/** Merge newly observed facts; omitted values preserve previously known truth. */
export async function mergeForgeWorkflowEvidence(
  processInstanceId: string,
  storyId: string,
  evidence: ForgeEvidenceMerge,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const clearReleaseFailure = evidence.releaseFailureResolved === true
  await q`
    insert into forge_workflow_evidence (
      process_instance_id, story_id, work_type, research_disposition,
      scout_required, root_cause_known, diagnosis_blocked, architecture_suspect, architecture_review_required,
      lead_decision, split_count, lead_routing, qa_review_required, qa_review_passed, qa_passed,
      failure_class, failed_release_stage, last_failure, publish_succeeded, migration_required,
      migration_files, dev_migration_applied, dev_migration_verified,
      prod_migration_applied, prod_migration_verified, derived_refresh_required,
      derived_models, derived_refresh_succeeded, derived_refresh_verified,
      deployment_required, deployment_succeeded, deployment_receipt,
      production_verified, production_verification_receipt, resume_target, candidate_sha, qa_verified_sha,
      published_sha, deployed_sha, production_verified_sha, findings, deployment_deferred_to_batch,
      negative_control_ran, negative_control_killing_assertion, gate_checks
    ) values (
      ${processInstanceId}, ${storyId}, ${evidence.workType ?? null},
      ${evidence.researchDisposition ?? null}, ${evidence.scoutRequired ?? null},
      ${evidence.rootCauseKnown ?? null}, ${evidence.diagnosisBlocked ?? null},
      ${evidence.architectureSuspect ?? null}, ${evidence.architectureReviewRequired ?? null}, ${evidence.leadDecision ?? null},
      ${evidence.leadDecision === 'SPLIT' ? evidence.splitCount ?? null : null}, ${evidence.leadRouting ? JSON.stringify(evidence.leadRouting) : null}, ${evidence.qaReviewRequired ?? null},
      ${evidence.qaReviewPassed ?? null}, ${evidence.qaPassed ?? null},
      ${evidence.failureClass ?? null}, ${evidence.failedReleaseStage ?? null}, ${evidence.lastFailure ?? null},
      ${evidence.publishSucceeded ?? null}, ${evidence.migrationRequired ?? null},
      ${evidence.migrationFiles === undefined ? null : JSON.stringify(evidence.migrationFiles)}::jsonb,
      ${evidence.devMigrationApplied ?? null}, ${evidence.devMigrationVerified ?? null},
      ${evidence.prodMigrationApplied ?? null}, ${evidence.prodMigrationVerified ?? null},
      ${evidence.derivedRefreshRequired ?? null},
      ${evidence.derivedModels === undefined ? null : JSON.stringify(evidence.derivedModels)}::jsonb,
      ${evidence.derivedRefreshSucceeded ?? null}, ${evidence.derivedRefreshVerified ?? null},
      ${evidence.deploymentRequired ?? null}, ${evidence.deploymentSucceeded ?? null},
      ${evidence.deploymentReceipt ?? null}, ${evidence.productionVerified ?? null},
      ${evidence.productionVerificationReceipt ?? null}, ${evidence.resumeTarget ?? null},
      ${evidence.candidateSha ?? null}, ${evidence.qaVerifiedSha ?? null},
      ${evidence.publishedSha ?? null}, ${evidence.deployedSha ?? null},
      ${evidence.productionVerifiedSha ?? null},
      ${evidence.findings === undefined ? null : JSON.stringify(evidence.findings)}::jsonb,
      ${evidence.deploymentDeferredToBatch ?? null},
      ${evidence.negativeControl?.ran ?? null},
      ${evidence.negativeControl ? JSON.stringify(evidence.negativeControl.killingAssertions) : null},
      ${evidence.gateChecks === undefined ? null : JSON.stringify(evidence.gateChecks)}::jsonb
    )
    on conflict (process_instance_id) do update set
      work_type = coalesce(excluded.work_type, forge_workflow_evidence.work_type),
      research_disposition = coalesce(excluded.research_disposition, forge_workflow_evidence.research_disposition),
      scout_required = coalesce(excluded.scout_required, forge_workflow_evidence.scout_required),
      root_cause_known = coalesce(excluded.root_cause_known, forge_workflow_evidence.root_cause_known),
      diagnosis_blocked = coalesce(excluded.diagnosis_blocked, forge_workflow_evidence.diagnosis_blocked),
      architecture_suspect = coalesce(excluded.architecture_suspect, forge_workflow_evidence.architecture_suspect),
      architecture_review_required = coalesce(excluded.architecture_review_required, forge_workflow_evidence.architecture_review_required),
      lead_decision = coalesce(excluded.lead_decision, forge_workflow_evidence.lead_decision),
      split_count = case when excluded.lead_decision is null then forge_workflow_evidence.split_count else excluded.split_count end,
      -- The accepted proposal follows its decision the same way: an explicit Lead
      -- write is authoritative, an unrelated write preserves what is known.
      lead_routing = case when excluded.lead_decision is null then forge_workflow_evidence.lead_routing else excluded.lead_routing end,
      qa_review_required = coalesce(excluded.qa_review_required, forge_workflow_evidence.qa_review_required),
      qa_review_passed = coalesce(excluded.qa_review_passed, forge_workflow_evidence.qa_review_passed),
      qa_passed = coalesce(excluded.qa_passed, forge_workflow_evidence.qa_passed),
      deployment_deferred_to_batch = coalesce(excluded.deployment_deferred_to_batch, forge_workflow_evidence.deployment_deferred_to_batch),
      negative_control_ran = coalesce(excluded.negative_control_ran, forge_workflow_evidence.negative_control_ran),
      negative_control_killing_assertion = coalesce(excluded.negative_control_killing_assertion, forge_workflow_evidence.negative_control_killing_assertion),
      gate_checks = coalesce(excluded.gate_checks, forge_workflow_evidence.gate_checks),
      -- An explicit release resolution clears the three markers together; every other
      -- write coalesces, so an unresolved failure keeps its markers.
      failure_class = case when ${clearReleaseFailure} then null
        else coalesce(excluded.failure_class, forge_workflow_evidence.failure_class) end,
      failed_release_stage = case when ${clearReleaseFailure} then null
        else coalesce(excluded.failed_release_stage, forge_workflow_evidence.failed_release_stage) end,
      last_failure = case when ${clearReleaseFailure} then null
        else coalesce(excluded.last_failure, forge_workflow_evidence.last_failure) end,
      publish_succeeded = coalesce(excluded.publish_succeeded, forge_workflow_evidence.publish_succeeded),
      migration_required = coalesce(excluded.migration_required, forge_workflow_evidence.migration_required),
      migration_files = coalesce(excluded.migration_files, forge_workflow_evidence.migration_files),
      dev_migration_applied = coalesce(excluded.dev_migration_applied, forge_workflow_evidence.dev_migration_applied),
      dev_migration_verified = coalesce(excluded.dev_migration_verified, forge_workflow_evidence.dev_migration_verified),
      prod_migration_applied = coalesce(excluded.prod_migration_applied, forge_workflow_evidence.prod_migration_applied),
      prod_migration_verified = coalesce(excluded.prod_migration_verified, forge_workflow_evidence.prod_migration_verified),
      derived_refresh_required = coalesce(excluded.derived_refresh_required, forge_workflow_evidence.derived_refresh_required),
      derived_models = coalesce(excluded.derived_models, forge_workflow_evidence.derived_models),
      derived_refresh_succeeded = coalesce(excluded.derived_refresh_succeeded, forge_workflow_evidence.derived_refresh_succeeded),
      derived_refresh_verified = coalesce(excluded.derived_refresh_verified, forge_workflow_evidence.derived_refresh_verified),
      deployment_required = coalesce(excluded.deployment_required, forge_workflow_evidence.deployment_required),
      deployment_succeeded = coalesce(excluded.deployment_succeeded, forge_workflow_evidence.deployment_succeeded),
      deployment_receipt = coalesce(excluded.deployment_receipt, forge_workflow_evidence.deployment_receipt),
      production_verified = coalesce(excluded.production_verified, forge_workflow_evidence.production_verified),
      production_verification_receipt = coalesce(excluded.production_verification_receipt, forge_workflow_evidence.production_verification_receipt),
      resume_target = coalesce(excluded.resume_target, forge_workflow_evidence.resume_target),
      candidate_sha = coalesce(excluded.candidate_sha, forge_workflow_evidence.candidate_sha),
      qa_verified_sha = coalesce(excluded.qa_verified_sha, forge_workflow_evidence.qa_verified_sha),
      published_sha = coalesce(excluded.published_sha, forge_workflow_evidence.published_sha),
      deployed_sha = coalesce(excluded.deployed_sha, forge_workflow_evidence.deployed_sha),
      production_verified_sha = coalesce(excluded.production_verified_sha, forge_workflow_evidence.production_verified_sha),
      findings = coalesce(excluded.findings, forge_workflow_evidence.findings),
      updated_at = now()
  `
}

/**
 * Record a batch release receipt onto every story the release carried.
 *
 * The receipt is DERIVED by `batchReleaseReceiptFromOutcome` from the actual
 * release outcome; this writer refuses null and writes nothing without one. It
 * owns exactly these three columns — the per-story deploy path keeps
 * `deployment_receipt` and `deployed_sha` — so one fact has exactly one writer.
 * Only rows whose RECORDED deferral names the released batch are touched, so a
 * receipt can never land on a story the batch did not carry.
 */
export async function recordForgeBatchReleaseReceipt(
  receipt: BatchReleaseReceipt | null | undefined,
  execute?: QueryExecutor,
): Promise<number> {
  if (!receipt) {
    throw new Error(
      'recordForgeBatchReleaseReceipt: refusing to write a batch release without a derived receipt',
    )
  }
  const q = execute ?? (await executor())
  const receiptId = `batch-release:${receipt.batch}:${receipt.releasedSha}`
  const releasedAt = new Date(receipt.releasedAt)
  let written = 0
  for (const storyId of receipt.storyIds) {
    const rows = await q`
      update forge_workflow_evidence
      set batch_released_sha = ${receipt.releasedSha},
          batch_released_at = ${releasedAt},
          batch_release_receipt = ${receiptId},
          updated_at = now()
      where story_id = ${storyId}
        and deployment_deferred_to_batch = ${receipt.batch}
      returning story_id
    `
    written += rows.length
  }
  return written
}

export async function readForgeWorkflowEvidence(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeGateEvidence> {
  const q = execute ?? (await executor())
  const rows = await q`
    select e.*
    from forge_workflow_evidence e
    join process_instances pi on pi.id = e.process_instance_id
    where e.story_id = ${storyId}
    order by (pi.status = 'active') desc, e.updated_at desc
    limit 1
  `
  const evidence = rows[0] ? mapForgeWorkflowEvidence(rows[0] as EvidenceRow) : {}

  // V11-S1: merge the durable repair/replan OBSERVER counts + last QA disposition
  // from the canonical story row so the engine's qa_failure_route decision can
  // route on them (the engine is the stop authority; the ledger only records).
  const ledger = (
    await q`
      select forge_repair_attempts, forge_replan_attempts, forge_last_qa_disposition
      from storyboard_story
      where id = ${storyId}
    `
  )[0] as
    | {
        forge_repair_attempts: number | null
        forge_replan_attempts: number | null
        forge_last_qa_disposition: string | null
      }
    | undefined
  if (ledger) {
    evidence.repairAttempts = Number(ledger.forge_repair_attempts ?? 0)
    evidence.replanAttempts = Number(ledger.forge_replan_attempts ?? 0)
    // ONLY a failure disposition is a routing fact. A stored PASS — now legal in the
    // column — must never be published as one, so classify against the one vocabulary
    // instead of casting the raw value into the routing enum.
    const stored = classifyStoredQaDisposition(ledger.forge_last_qa_disposition)
    if (stored === 'REPAIR' || stored === 'REPLAN' || stored === 'ESCALATE') {
      evidence.disposition = stored
    }
  }
  return evidence
}
