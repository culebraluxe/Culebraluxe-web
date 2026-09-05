import type { ForgeGateEvidence } from '../workflow_app/forge/forge-facts'
import type { QueryExecutor, QueryRow } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

type EvidenceRow = QueryRow & Record<string, unknown>

const value = <T>(row: EvidenceRow, key: string): T | undefined => {
  const current = row[key]
  return current === null || current === undefined ? undefined : (current as T)
}

const stringArray = (row: EvidenceRow, key: string): string[] | undefined => {
  const current = row[key]
  if (current === null || current === undefined) return undefined
  const parsed = typeof current === 'string' ? JSON.parse(current) : current
  return Array.isArray(parsed) && parsed.every((item) => typeof item === 'string')
    ? parsed
    : undefined
}

export function mapForgeWorkflowEvidence(row: EvidenceRow): ForgeGateEvidence {
  return {
    workType: value(row, 'work_type'),
    researchDisposition: value(row, 'research_disposition'),
    scoutRequired: value(row, 'scout_required'),
    rootCauseKnown: value(row, 'root_cause_known'),
    diagnosisBlocked: value(row, 'diagnosis_blocked'),
    architectureSuspect: value(row, 'architecture_suspect'),
    leadDecision: value(row, 'lead_decision'),
    splitCount: value(row, 'split_count'),
    qaReviewRequired: value(row, 'qa_review_required'),
    qaReviewPassed: value(row, 'qa_review_passed'),
    qaPassed: value(row, 'qa_passed'),
    failureClass: value(row, 'failure_class'),
    failedReleaseStage: value(row, 'failed_release_stage'),
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
  }
}

/** Merge newly observed facts; omitted values preserve previously known truth. */
export async function mergeForgeWorkflowEvidence(
  processInstanceId: string,
  storyId: string,
  evidence: ForgeGateEvidence,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    insert into forge_workflow_evidence (
      process_instance_id, story_id, work_type, research_disposition,
      scout_required, root_cause_known, diagnosis_blocked, architecture_suspect,
      lead_decision, split_count, qa_review_required, qa_review_passed, qa_passed,
      failure_class, failed_release_stage, publish_succeeded, migration_required,
      migration_files, dev_migration_applied, dev_migration_verified,
      prod_migration_applied, prod_migration_verified, derived_refresh_required,
      derived_models, derived_refresh_succeeded, derived_refresh_verified,
      deployment_required, deployment_succeeded, deployment_receipt,
      production_verified, production_verification_receipt, resume_target, candidate_sha, qa_verified_sha,
      published_sha, deployed_sha, production_verified_sha
    ) values (
      ${processInstanceId}, ${storyId}, ${evidence.workType ?? null},
      ${evidence.researchDisposition ?? null}, ${evidence.scoutRequired ?? null},
      ${evidence.rootCauseKnown ?? null}, ${evidence.diagnosisBlocked ?? null},
      ${evidence.architectureSuspect ?? null}, ${evidence.leadDecision ?? null},
      ${evidence.splitCount ?? null}, ${evidence.qaReviewRequired ?? null},
      ${evidence.qaReviewPassed ?? null}, ${evidence.qaPassed ?? null},
      ${evidence.failureClass ?? null}, ${evidence.failedReleaseStage ?? null},
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
      ${evidence.productionVerifiedSha ?? null}
    )
    on conflict (process_instance_id) do update set
      work_type = coalesce(excluded.work_type, forge_workflow_evidence.work_type),
      research_disposition = coalesce(excluded.research_disposition, forge_workflow_evidence.research_disposition),
      scout_required = coalesce(excluded.scout_required, forge_workflow_evidence.scout_required),
      root_cause_known = coalesce(excluded.root_cause_known, forge_workflow_evidence.root_cause_known),
      diagnosis_blocked = coalesce(excluded.diagnosis_blocked, forge_workflow_evidence.diagnosis_blocked),
      architecture_suspect = coalesce(excluded.architecture_suspect, forge_workflow_evidence.architecture_suspect),
      lead_decision = coalesce(excluded.lead_decision, forge_workflow_evidence.lead_decision),
      split_count = coalesce(excluded.split_count, forge_workflow_evidence.split_count),
      qa_review_required = coalesce(excluded.qa_review_required, forge_workflow_evidence.qa_review_required),
      qa_review_passed = coalesce(excluded.qa_review_passed, forge_workflow_evidence.qa_review_passed),
      qa_passed = coalesce(excluded.qa_passed, forge_workflow_evidence.qa_passed),
      failure_class = coalesce(excluded.failure_class, forge_workflow_evidence.failure_class),
      failed_release_stage = coalesce(excluded.failed_release_stage, forge_workflow_evidence.failed_release_stage),
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
      updated_at = now()
  `
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
  return rows[0] ? mapForgeWorkflowEvidence(rows[0] as EvidenceRow) : {}
}
