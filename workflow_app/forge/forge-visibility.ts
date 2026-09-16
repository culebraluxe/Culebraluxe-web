import { engineSql } from '../engine-client'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { latestOpenForgeHold, listForgeHolds, type ForgeStoryHold } from '../../db/forge-hold'
import { listStoryRuns } from '../../db/storyboard'
import type { ForgeGateEvidence } from './forge-facts'

export const FORGE_SPEND_UNRECORDED_LANE = 'unrecorded'

/**
 * One spend quantity as read from the durable run rows. `null` means UNMEASURED,
 * never zero: a measured zero and an unmeasured zero are different facts.
 */
export type ForgeSpendDimension = {
  tokensInput: number | null
  tokensOutput: number | null
  costUsd: number | null
  costWidgets: number | null
  costSource: string | null
}

export type ForgeLaneSpend = {
  lane: string
  runs: number
  spend: ForgeSpendDimension
}

export type ForgeSpendBlock = {
  lanes: ForgeLaneSpend[]
  total: ForgeSpendDimension
}

/** The durable-run fields the spend block reads. `StoryRun` structurally satisfies this. */
export type ForgeSpendRun = {
  runType: string | null
  tokensInput?: number | null
  tokensOutput?: number | null
  costUsd?: number | null
  costWidgets?: number | null
  costSource?: string | null
}

/**
 * Sum of the KNOWN values. All-unknown is `null` (never 0); a known 0 stays 0.
 * `Number.isFinite` keeps a NaN or non-number out of the total instead of poisoning it.
 */
function sumKnown(values: Array<number | null | undefined>): number | null {
  const known = values.filter((v): v is number => typeof v === 'number' && Number.isFinite(v))
  if (known.length === 0) return null
  return known.reduce((sum, v) => sum + v, 0)
}

/** Distinct non-blank sources, sorted and joined; `null` when no run recorded one. */
function mergeCostSources(values: Array<string | null | undefined>): string | null {
  const known = [
    ...new Set(values.map((v) => v?.trim()).filter((v): v is string => Boolean(v))),
  ].sort()
  return known.length === 0 ? null : known.join('+')
}

function spendDimension(runs: readonly ForgeSpendRun[]): ForgeSpendDimension {
  return {
    tokensInput: sumKnown(runs.map((r) => r.tokensInput)),
    tokensOutput: sumKnown(runs.map((r) => r.tokensOutput)),
    costUsd: sumKnown(runs.map((r) => r.costUsd)),
    costWidgets: sumKnown(runs.map((r) => r.costWidgets)),
    costSource: mergeCostSources(runs.map((r) => r.costSource)),
  }
}

/**
 * The spend of a story's own runs, per lane and in total, from the durable run rows.
 *
 * Lane identity is `run_type` (the Forge lane: scout|architect|lead|smith|inspector|
 * assay|archive|night). A run whose lane is null/blank lands in an `unrecorded`
 * bucket and is never dropped — an unrecorded lane is a fact, not a zero.
 *
 * Pure and synchronous so the acceptance fixture is a unit test, not a screenshot.
 */
export function forgeSpendBlock(runs: readonly ForgeSpendRun[]): ForgeSpendBlock {
  const buckets = new Map<string, ForgeSpendRun[]>()
  for (const run of runs) {
    const lane = (run.runType ?? '').trim() || FORGE_SPEND_UNRECORDED_LANE
    const bucket = buckets.get(lane)
    if (bucket) bucket.push(run)
    else buckets.set(lane, [run])
  }
  const lanes = [...buckets.entries()]
    .map(([lane, rows]) => ({ lane, runs: rows.length, spend: spendDimension(rows) }))
    .sort((a, b) => a.lane.localeCompare(b.lane))
  return { lanes, total: spendDimension(runs) }
}

export type ForgeVisibilitySnapshot = {
  storyId: string
  instance: { id: string | null; status: string | null; currentNodes: string[] }
  roleTasks: Array<{
    taskId: string
    nodeId: string
    status: string
    assignee: string | null
    claimAgeMs: number | null
  }>
  execution: Array<{ nodeId: string; status: string; workerId: string; storyRunId: string | null }>
  evidence: ForgeGateEvidence
  shaChain: {
    candidateSha: string | null
    qaVerifiedSha: string | null
    publishedSha: string | null
    deployedSha: string | null
    productionVerifiedSha: string | null
  }
  shaEquality: ReturnType<typeof forgeVisibilityEquality>
  receipts: { deploymentReceipt: string | null; productionVerificationReceipt: string | null }
  failedReleaseStage: ForgeGateEvidence['failedReleaseStage'] | null
  commandVisits: Array<{ nodeId: string; visitSequence: number; outcome: string | null }>
  splitBranches: Array<{
    index: number | null
    count: number | null
    status: string
    storyRunId: string | null
    costWidgets: number | null
  }>
  holds: unknown[]
  hold: ForgeHoldLine | null
  spend: ForgeSpendBlock
  divergenceWarning: string | null
}

export const FORGE_HOLD_UNKNOWN = 'unknown'

export type ForgeHoldLine = {
  reason: string
  originatingNode: string | null
  failureClass: string | null
  resumeTarget: string | null
  since: string | null
  processInstanceId: string | null
}

export function forgeHoldLine(hold: ForgeStoryHold | null | undefined): ForgeHoldLine | null {
  if (!hold) return null
  const reason = hold.reason?.trim()
  return {
    reason: reason ? reason : FORGE_HOLD_UNKNOWN,
    originatingNode: hold.originatingNode ?? null,
    failureClass: hold.failureClass ?? null,
    resumeTarget: hold.resumeTarget ?? null,
    since: hold.since ?? null,
    processInstanceId: hold.processInstanceId ?? null,
  }
}

const shaKeys = [
  'candidate_sha',
  'qa_verified_sha',
  'published_sha',
  'deployed_sha',
  'production_verified_sha',
] as const
const receiptKeys = ['deployment_receipt', 'production_verification_receipt'] as const

function norm(value: string | null | undefined): string | null {
  const sha = value?.trim().toLowerCase() ?? ''
  return /^[0-9a-f]{7,64}$/.test(sha) ? sha : null
}

export function forgeVisibilityEquality(chain: {
  candidateSha: string | null
  qaVerifiedSha: string | null
  publishedSha: string | null
  deployedSha: string | null
  productionVerifiedSha: string | null
}): {
  candidateEqualsQa: boolean
  publishedEqualsCandidate: boolean
  deployedEqualsPublished: boolean
  productionEqualsPublished: boolean
} {
  const candidate = norm(chain.candidateSha)
  const qa = norm(chain.qaVerifiedSha)
  const published = norm(chain.publishedSha)
  const deployed = norm(chain.deployedSha)
  const production = norm(chain.productionVerifiedSha)
  return {
    candidateEqualsQa: Boolean(candidate && qa && candidate === qa),
    publishedEqualsCandidate: Boolean(candidate && published && candidate === published),
    deployedEqualsPublished: Boolean(published && deployed && published === deployed),
    productionEqualsPublished: Boolean(published && production && published === production),
  }
}

export async function forgeVisibilitySnapshot(
  storyId: string,
): Promise<ForgeVisibilitySnapshot> {
  const instance = await engineSql()`
    select pi.id, pi.status
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'story' and pi.subject_id = ${storyId} and pd.key = 'FORGE_SDLC'
    order by pi.created_at desc limit 1
  `
  const inst = instance[0] as { id?: string; status?: string } | undefined
  const instanceId = inst?.id ?? null

  let currentNodes: string[] = []
  let roleTasks: ForgeVisibilitySnapshot['roleTasks'] = []
  let execution: ForgeVisibilitySnapshot['execution'] = []
  let commandVisits: ForgeVisibilitySnapshot['commandVisits'] = []
  let splitBranches: ForgeVisibilitySnapshot['splitBranches'] = []
  if (instanceId) {
    const tokens = await engineSql()`
      select node_id from tokens where process_instance_id = ${instanceId} and status = 'active'
    `
    currentNodes = (tokens as Array<{ node_id: string }>).map((t) => t.node_id)
    const tasks = await engineSql()`
      select t.id, tk.node_id, t.status, t.assignee, t.claimed_at, t.updated_at, t.form_data
      from tasks t join tokens tk on tk.id = t.token_id
      where t.process_instance_id = ${instanceId}
        and t.status in ('ready','reserved','in_progress')
    `
    const now = Date.now()
    roleTasks = (
      tasks as Array<{
        id: string
        node_id: string
        status: string
        assignee: string | null
        claimed_at?: string | null
        updated_at?: string | null
        form_data?: Record<string, unknown> | null
      }>
    ).map((r) => {
      const claimed = r.claimed_at ?? r.updated_at
      const claimAgeMs =
        r.status !== 'ready' && claimed ? Math.max(0, now - new Date(claimed).getTime()) : null
      return {
        taskId: r.id,
        nodeId: r.node_id,
        status: r.status,
        assignee: r.assignee,
        claimAgeMs,
      }
    })
    const ex = await engineSql()`
      select node_id, status, worker_id, story_run_id
      from forge_engine_task_execution
      where process_instance_id = ${instanceId}
      order by created_at desc limit 20
    `
    execution = (
      ex as Array<{ node_id: string; status: string; worker_id: string; story_run_id: string | null }>
    ).map((r) => ({ nodeId: r.node_id, status: r.status, workerId: r.worker_id, storyRunId: r.story_run_id }))

    try {
      const visits = await engineSql()`
        select node_id, visit_sequence, outcome
        from process_commands
        where process_instance_id = ${instanceId}
        order by visit_sequence desc
        limit 40
      `
      commandVisits = (
        visits as Array<{ node_id: string; visit_sequence: number; outcome: string | null }>
      ).map((r) => ({
        nodeId: r.node_id,
        visitSequence: Number(r.visit_sequence),
        outcome: r.outcome,
      }))
    } catch {
      commandVisits = []
    }

    splitBranches = (
      tasks as Array<{ node_id: string; status: string; form_data?: Record<string, unknown> | null }>
    )
      .filter((r) => r.node_id === 'smith_split_work')
      .map((r) => ({
        index: Number(r.form_data?.splitBranchIndex ?? NaN) || null,
        count: Number(r.form_data?.splitBranchCount ?? NaN) || null,
        status: r.status,
        storyRunId:
          execution.find((row) => row.nodeId === 'smith_split_work')?.storyRunId ?? null,
        costWidgets: null,
      }))
  }

  const evidence = await readForgeWorkflowEvidence(storyId)
  const e = evidence ?? {}
  const spend = forgeSpendBlock(await listStoryRuns(storyId))
  const shaChain: ForgeVisibilitySnapshot['shaChain'] = {
    candidateSha: e.candidateSha ?? null,
    qaVerifiedSha: e.qaVerifiedSha ?? null,
    publishedSha: e.publishedSha ?? null,
    deployedSha: e.deployedSha ?? null,
    productionVerifiedSha: e.productionVerifiedSha ?? null,
  }
  const receipts: ForgeVisibilitySnapshot['receipts'] = {
    deploymentReceipt: e.deploymentReceipt ?? null,
    productionVerificationReceipt: e.productionVerificationReceipt ?? null,
  }

  let divergenceWarning: string | null = null
  const story = await engineSql()`
    select status from storyboard_story where id = ${storyId}
  `
  const storyStatus = (story[0] as { status?: string } | undefined)?.status ?? null
  if (storyStatus === 'Complete' && inst && inst.status !== 'completed') {
    divergenceWarning = 'Storyboard Complete but engine instance is not completed'
  } else if (inst && inst.status === 'completed' && storyStatus !== 'Complete') {
    divergenceWarning = `Engine completed but Storyboard status is '${storyStatus ?? '(none)'}'`
  }

  return {
    storyId,
    instance: { id: instanceId, status: inst?.status ?? null, currentNodes },
    roleTasks,
    execution,
    evidence: e,
    shaChain,
    shaEquality: forgeVisibilityEquality(shaChain),
    receipts,
    failedReleaseStage: e.failedReleaseStage ?? null,
    commandVisits,
    splitBranches,
    holds: await listForgeHolds(storyId),
    hold: forgeHoldLine(await latestOpenForgeHold(storyId)),
    spend,
    divergenceWarning,
  }
}

export { shaKeys, receiptKeys }
