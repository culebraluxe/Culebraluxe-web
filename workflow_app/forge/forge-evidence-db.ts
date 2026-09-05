import { engineSql } from '../engine-client'
import type { ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 #1 — durable gate evidence reader (maps the Neon run tables).
//
// The engine instance variables carry workType (set at start). Everything else
// a decision gate needs (qaPassed, publishSucceeded, deploymentSucceeded,
// productionVerified, migrationRequired, ...) is the OUTPUT of a role run that
// the Forge runtime records as a storyboard_story_run row (run_type +
// result_status [+ commit_hash]) and an agent_work_item (role/state/attempts).
//
// This reader maps those DURABLE rows back to ForgeGateEvidence so readFacts is
// stable across separate engine calls/resumes — no in-memory evidence. Mapping
// contract (run_type -> gate fact):
//   qa|assay complete/success  -> qaPassed true
//   publish complete           -> publishSucceeded true
//   deploy complete            -> deploymentSucceeded true
//   smoke|production complete  -> productionVerified true
//   migration (any) complete   -> migrationRequired true + verified
//
// The real role runner (#5) records these rows; this is the reader half of the
// durable-evidence path. DB-free callers inject nothing; it queries the shared
// engine/app database via engineSql().
// ---------------------------------------------------------------------------

function isClean(status: string | null | undefined): boolean {
  return /complete|success|pass/i.test(status ?? '')
}

export type ForgeRunRowShape = {
  run_type: string | null
  result_status: string | null
  commit_hash: string | null
}

/** Pure mapping of run-table rows -> gate evidence (unit-testable, DB-free). */
export function mapRunsToGateEvidence(rows: ForgeRunRowShape[]): ForgeGateEvidence {
  const evidence: ForgeGateEvidence = {}
  let sawQa = false
  let sawPublish = false
  let sawMigration = false
  let sawDeploy = false
  let sawSmoke = false

  for (const r of rows) {
    const rt = (r.run_type ?? '').toLowerCase()
    const clean = isClean(r.result_status)
    if (!rt) continue
    if (rt.includes('qa') || rt.includes('assay')) {
      sawQa = true
      if (clean) evidence.qaPassed = true
    } else if (rt.includes('publish')) {
      sawPublish = true
      if (clean) evidence.publishSucceeded = true
    } else if (rt.includes('deploy')) {
      sawDeploy = true
      if (clean) evidence.deploymentSucceeded = true
    } else if (rt.includes('smoke') || rt.includes('production')) {
      sawSmoke = true
      if (clean) evidence.productionVerified = true
    } else if (rt.includes('migrat')) {
      sawMigration = true
      if (clean) evidence.migrationRequired = true
    }
  }

  if (sawQa && evidence.qaPassed !== true) evidence.qaPassed = false
  if (sawPublish && evidence.publishSucceeded !== true) evidence.publishSucceeded = false
  if (sawMigration && evidence.migrationRequired !== true) evidence.migrationRequired = false
  if (sawDeploy && evidence.deploymentSucceeded !== true) evidence.deploymentSucceeded = false
  if (sawSmoke && evidence.productionVerified !== true) evidence.productionVerified = false

  return evidence
}

export async function readStoryGateEvidence(
  storyId: string,
): Promise<ForgeGateEvidence> {
  const rows = (await engineSql()`
    select run_type, result_status, commit_hash
    from storyboard_story_run
    where story_id = ${storyId}
    order by started_at desc nulls last, created_at desc
    limit 50
  `) as ForgeRunRowShape[]
  return mapRunsToGateEvidence(rows)
}

/**
 * The durable evidence reader wired as ApplicationPort.readFacts: query the run
 * table for a story and project its gate evidence.
 */
export function createStoryGateEvidenceReader(): (storyId: string) => Promise<ForgeGateEvidence> {
  return readStoryGateEvidence
}
