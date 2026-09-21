// ---------------------------------------------------------------------------
// The batch release can see the slice it is releasing.
//
// A story whose deployment is deferred to batch N belongs to slice N — even when
// nothing has been published yet. Membership is the RECORDED deferral target
// (forge_workflow_evidence.deployment_deferred_to_batch); `storyboard_story.batch`
// is a display attribute and never grants membership. A row with no recorded
// deferral belongs to no slice: null, never a guess.
//
// A deferred story that is already published is reported as
// `published-and-undeployed`, so the slice can be released deliberately.
//
// Pure module: no database, no network, no filesystem, no clock.
// ---------------------------------------------------------------------------

import { isRecordedDeploymentDeferral } from '@/legacy/workflow_app/forge/forge-release-receipt'

/** The release state of one story, derived from durable evidence only. */
export type ReleaseState =
  | 'deployed'
  | 'published-and-undeployed'
  | 'deferred'
  | 'qa-passed-unrecorded'
  | 'pending'

/**
 * One story as the release view reads it. The caller owns driver normalization;
 * this module never touches a database.
 */
export type BatchSliceRow = {
  id: string
  batch: number | null
  status: string
  qaPassed: boolean | null
  publishedSha: string | null
  deployedSha: string | null
  productionVerified: boolean | null
  deploymentReceipt: string | null
  deploymentDeferredToBatch: number | null
}

export type BatchSliceEntry = {
  id: string
  batch: number
  state: ReleaseState
  publishedSha: string | null
}

/**
 * The slice a row belongs to: the recorded deferral target, or null when the row
 * carries no deferral. A null deferral is never coerced to 0 and never guessed
 * from `row.batch`.
 */
export function sliceOf(row: BatchSliceRow): number | null {
  return isRecordedDeploymentDeferral(row.deploymentDeferredToBatch)
    ? (row.deploymentDeferredToBatch as number)
    : null
}

/**
 * State precedence, highest first: a recorded deployment outranks a deferral;
 * a deferred story that is already published is published-and-undeployed; then a
 * plain deferral; then a QA pass whose deployment was never recorded; then the
 * story's own status is the fallback.
 */
export function releaseState(row: BatchSliceRow): ReleaseState {
  if (row.deploymentReceipt || row.productionVerified) return 'deployed'
  if (sliceOf(row) !== null) {
    return row.publishedSha ? 'published-and-undeployed' : 'deferred'
  }
  if (row.qaPassed) return 'qa-passed-unrecorded'
  return 'pending'
}

/** Every story whose deployment is deferred to `batch`, in input order. */
export function sliceForBatch(rows: readonly BatchSliceRow[], batch: number): BatchSliceEntry[] {
  const entries: BatchSliceEntry[] = []
  for (const row of rows) {
    const slice = sliceOf(row)
    if (slice === null || slice !== batch) continue
    entries.push({
      id: row.id,
      batch: slice,
      state: releaseState(row),
      publishedSha: row.publishedSha,
    })
  }
  return entries
}
