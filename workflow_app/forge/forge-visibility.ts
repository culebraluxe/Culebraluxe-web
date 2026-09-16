import { engineSql } from '../engine-client'
import { sql } from '../../db/client'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { latestOpenForgeHold, listForgeHolds, type ForgeStoryHold } from '../../db/forge-hold'
import { listStoryRuns } from '../../db/storyboard'
import { DEFAULT_REPAIR_BUDGET } from './qa-repair-policy'
import type { ForgeGateEvidence } from './forge-facts'

export const FORGE_SPEND_UNRECORDED_LANE = 'unrecorded'

/**
 * The label an absent `run_type` reads as. A missing lane is a fact, not a blank:
 * it is named `Unrecorded` rather than an empty string that reads like "no lane".
 */
export const FORGE_LANE_LABEL_UNRECORDED = 'Unrecorded'

/**
 * The human lane label for each `run_type` the engine writes. The view names the
 * lane in words instead of the raw token. `inspector` and `assay` are both the QA
 * capability, so they share one label; `dev_ops` is here even though the Lane=
 * envelope regex omits it, because it reaches `run_type` through the role fallback.
 */
const FORGE_LANE_LABELS: Record<string, string> = {
  scout: 'Scout',
  architect: 'Architect',
  lead: 'Lead',
  smith: 'Smith',
  inspector: 'QA',
  assay: 'QA',
  archive: 'Archive',
  night: 'Night',
  dev_ops: 'DEV_OPS',
}

/**
 * The lane label for a run. Pure and synchronous so the acceptance fixture is a
 * unit test, not a screenshot. An unknown token is reported verbatim (trimmed),
 * never guessed; an absent token reads `Unrecorded`, never an empty string.
 */
export function forgeLaneLabel(runType: string | null | undefined): string {
  const token = (runType ?? '').trim()
  if (!token) return FORGE_LANE_LABEL_UNRECORDED
  return FORGE_LANE_LABELS[token] ?? token
}

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
  label: string
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
    .map(([lane, rows]) => ({
      lane,
      label: forgeLaneLabel(rows[0]?.runType),
      runs: rows.length,
      spend: spendDimension(rows),
    }))
    .sort((a, b) => a.lane.localeCompare(b.lane))
  return { lanes, total: spendDimension(runs) }
}

/**
 * One bounded-autonomy budget as used against its cap. `used` and `cap` are `null`
 * when the durable counter or the engine cap is unknown — an unmeasured value is not
 * a zero. `remaining` is `null` whenever either is unknown, otherwise `max(0, cap-used)`;
 * `exhausted` is `remaining === 0` when known, and `null` when it is not. "We do not
 * know" and "none left" are different facts, and only the second is a hold.
 */
export type ForgeBudgetDimension = {
  used: number | null
  cap: number | null
  remaining: number | null
  exhausted: boolean | null
}

/** The repair and replan budgets of one story, each against its engine cap. */
export type ForgeRepairBudgetBlock = {
  repair: ForgeBudgetDimension
  replan: ForgeBudgetDimension
}

function knownCount(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function budgetDimension(
  used: number | null | undefined,
  cap: number | null | undefined,
): ForgeBudgetDimension {
  const u = knownCount(used)
  const c = knownCount(cap)
  if (u === null || c === null) return { used: u, cap: c, remaining: null, exhausted: null }
  const remaining = Math.max(0, c - u)
  return { used: u, cap: c, remaining, exhausted: remaining === 0 }
}

/**
 * The repair/replan budget of a story: the durable attempt counters read against the
 * engine caps. Pure and synchronous so the acceptance fixture is a unit test, not a
 * screenshot. An absent counter is `null`, never coerced to 0.
 */
export function forgeRepairBudgetBlock(input: {
  repairAttempts?: number | null
  replanAttempts?: number | null
  maxRepairAttempts?: number | null
  maxReplanAttempts?: number | null
}): ForgeRepairBudgetBlock {
  return {
    repair: budgetDimension(input.repairAttempts, input.maxRepairAttempts),
    replan: budgetDimension(input.replanAttempts, input.maxReplanAttempts),
  }
}

/**
 * The snapshot's wiring: the durable evidence counters against the engine's own caps.
 * This is the exact call `forgeVisibilitySnapshot` makes, kept pure so the wiring
 * itself is under the frozen fixture rather than only the dimension math.
 */
export function forgeRepairBudgetFromEvidence(evidence: {
  repairAttempts?: number | null
  replanAttempts?: number | null
}): ForgeRepairBudgetBlock {
  return forgeRepairBudgetBlock({
    repairAttempts: evidence.repairAttempts,
    replanAttempts: evidence.replanAttempts,
    maxRepairAttempts: DEFAULT_REPAIR_BUDGET.maxRepairAttempts,
    maxReplanAttempts: DEFAULT_REPAIR_BUDGET.maxReplanAttempts,
  })
}

export const FORGE_QA_ASSAY_ARTIFACT_KIND = 'qa-assay-evidence'

/** One frozen command and the exit code the assay measured for it. */
export type ForgeQaCommandResult = {
  command: string
  exitCode: number | null
}

/**
 * The QA verdict of ONE run. The block is `null` — NO VERDICT — when the run has no
 * QA artifact of its own. It never falls back to another run or to the story's
 * history: a run that measured nothing says so, rather than inheriting a verdict.
 *
 * The block carries no SHA on purpose. Artifact identity lives in `shaChain`; a
 * verdict is a measurement, not an identity, and a stale SHA beside a fresh verdict
 * is how one run's pass got read as another's.
 */
export type ForgeQaVerdict = {
  runId: string
  verdict: 'PASS' | 'FAIL'
  qaPassed: boolean
  commands: ForgeQaCommandResult[]
}

/** The durable artifact fields the verdict block reads. */
export type ForgeQaArtifactRow = {
  storyRunId: string | null
  kind: string | null
  verdict: string | null
  detail: unknown
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return value as Record<string, unknown>
  }
  if (typeof value === 'string') {
    try {
      return asRecord(JSON.parse(value))
    } catch {
      return null
    }
  }
  return null
}

function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : []
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

/**
 * The verdict of the run `runId`, from that run's OWN QA artifacts, newest first.
 *
 * Pure and synchronous so the acceptance fixture is a unit test, not a screenshot.
 * A row for another run, or of another kind, is not this run's verdict — and when no
 * row matches, the answer is `null`, never a best-of-history pass.
 */
export function forgeQaVerdictBlock(
  runId: string,
  rows: readonly ForgeQaArtifactRow[],
): ForgeQaVerdict | null {
  const run = (runId ?? '').trim()
  if (!run) return null
  const artifact = rows.find(
    (r) =>
      (r.storyRunId ?? '').trim() === run &&
      (r.kind ?? '').trim() === FORGE_QA_ASSAY_ARTIFACT_KIND,
  )
  if (!artifact) return null
  const verdict = (artifact.verdict ?? '').trim().toUpperCase()
  // A row with no readable verdict is NOT a FAIL — it is no verdict at all.
  if (verdict !== 'PASS' && verdict !== 'FAIL') return null
  const detail = asRecord(artifact.detail)
  const required = stringList(detail?.requiredCommands)
  const results = Array.isArray(detail?.commandResults) ? detail.commandResults : []
  const byCommand = new Map<string, number | null>()
  const measured: ForgeQaCommandResult[] = []
  for (const raw of results) {
    const rec = asRecord(raw)
    if (!rec || typeof rec.command !== 'string' || !rec.command) continue
    byCommand.set(rec.command, numberOrNull(rec.exitCode))
    measured.push({ command: rec.command, exitCode: numberOrNull(rec.exitCode) })
  }
  // The FROZEN commands are the truth: every one is named, and one with no measured
  // result reads `null` — an unmeasured exit code is not a zero.
  const commands =
    required.length > 0
      ? required.map((command) => ({ command, exitCode: byCommand.get(command) ?? null }))
      : measured
  return { runId: run, verdict, qaPassed: verdict === 'PASS', commands }
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
  repairBudget: ForgeRepairBudgetBlock
  qaVerdict: ForgeQaVerdict | null
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

/**
 * The run's OWN QA artifacts, newest first, with the machine `detail` the assay wrote.
 * Scoped to `story_run_id`, so a verdict can never answer for another run. The read
 * lives in the verdict block's own surface; `forgeQaVerdictBlock` owns the shape.
 */
async function readQaArtifactsForRun(runId: string): Promise<ForgeQaArtifactRow[]> {
  const rows = (await sql`
    select story_run_id, kind, verdict, detail
    from forge_tool_artifact
    where story_run_id = ${runId}
    order by created_at desc
  `) as Array<{
    story_run_id: string | null
    kind: string | null
    verdict: string | null
    detail: unknown
  }>
  return rows.map((r) => ({
    storyRunId: r.story_run_id,
    kind: r.kind,
    verdict: r.verdict,
    detail: r.detail,
  }))
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

  // THE VERDICT BELONGS TO THE RUN WE ARE VIEWING. The newest execution row that names
  // a run is the run in flight; no run means no verdict — never the story's history.
  const viewedRunId = execution.find((row) => row.storyRunId)?.storyRunId ?? null
  const qaVerdict = viewedRunId
    ? forgeQaVerdictBlock(viewedRunId, await readQaArtifactsForRun(viewedRunId))
    : null

  const evidence = await readForgeWorkflowEvidence(storyId)
  const e = evidence ?? {}
  const spend = forgeSpendBlock(await listStoryRuns(storyId))
  const repairBudget = forgeRepairBudgetFromEvidence(e)
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
    repairBudget,
    qaVerdict,
    divergenceWarning,
  }
}

export { shaKeys, receiptKeys }
