// ---------------------------------------------------------------------------
// ENG-FORGE-DOCTOR-01 — the control plane in one command.
//
//   pnpm forge:doctor
//
// Answers the operator question "is the control plane clear?" before a test
// run, so nobody has to hand-write the query again. `forge:clean` is the
// writer; this is its READ-ONLY sibling, so an operator can look before
// deciding to clean.
//
// READ-ONLY IS A HARD REQUIREMENT: this file issues no update, no insert and
// no claim. Every fact it prints is read from data that already exists — the
// engine ledger, the agent_work queue, the board/batch tables, forge_decision,
// the ROI read model and the learn-loop anchor — plus the scheduled worker's
// own invocation log. No new query and no new meter is introduced.
//
// The rendering is a PURE function in
// workflow_app/forge/forge-doctor-report.ts with a unit test, so the
// empty-control-plane, board-drifted and worker-failing cases are fixtures
// rather than live probes.
// ---------------------------------------------------------------------------

import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { listActiveAgentWorkItems } from '@/db/agent-work'
import { getStagingBatch } from '@/db/forge-batch'
import { listActiveDecisions } from '@/db/forge-decision'
import { listEngineQueuedCards, listEngineRunCards } from '@/db/forge-engine-task-execution'
import { listRoiAttempts, ROI_DEFAULT_WINDOW_DAYS } from '@/db/forge-roi'
import { readForgeWorkflowEvidence } from '@/db/forge-workflow-evidence'
import { listStoryboardRuns, listStoryboardStories, type StoryRun } from '@/db/storyboard'
import { describeRoiRow, summarizeRoi } from '@/lib/forge-roi'
import { readLearnAnchor } from '@/agent-runtime/learn-loop'
import {
  checkQaRunVerdictConsistency,
  isQaRunType,
  renderQaConsistencyLine,
  type QaConsistencyResult,
} from '@/workflow_app/forge/forge-qa-consistency'
import {
  renderForgeDoctorReport,
  type DoctorControlPlane,
  type DoctorOldestClaim,
  type DoctorPostcard,
  type DoctorWorkerLiveness,
} from '@/workflow_app/forge/forge-doctor-report'

/** The engine ledger's own terminal vocabulary. Anything else is still open. */
const TERMINAL_ENGINE_STATUSES = new Set(['completed', 'failed', 'interrupted'])

/**
 * Timestamps cross this boundary in two shapes: the engine ledger returns
 * `::text` (space-separated), while `agent_work` passes the driver value
 * through unnormalized (a `Date`). Accept both rather than assume one.
 */
function parseDbTime(value: unknown): number | null {
  if (value == null) return null
  if (value instanceof Date) return Number.isFinite(value.getTime()) ? value.getTime() : null
  if (typeof value === 'number') return Number.isFinite(value) ? value : null
  const text = String(value)
  if (!text) return null
  const normalized = text.includes('T') ? text : `${text.replace(' ', 'T')}Z`
  const ms = Date.parse(normalized)
  return Number.isFinite(ms) ? ms : null
}

async function gatherControlPlane(): Promise<DoctorControlPlane> {
  const [runCards, queuedCards, activeWork] = await Promise.all([
    listEngineRunCards(50),
    listEngineQueuedCards(50),
    listActiveAgentWorkItems(20),
  ])

  const openRunCards = runCards.filter((card) => !TERMINAL_ENGINE_STATUSES.has(card.status))
  // A stale claim is abandoned, not running (active decision: abandoned-claim-is-not-running).
  const engineClaims = openRunCards.filter((card) => !card.stale)
  const activeClaims = engineClaims.length + activeWork.length

  const now = Date.now()
  const claims: DoctorOldestClaim[] = []
  for (const item of activeWork) {
    // `agent_work_item` is the ledger that actually carries claimed_at.
    const base = parseDbTime(item.claimedAt) ?? parseDbTime(item.updatedAt)
    if (base !== null) claims.push({ ledger: 'agent_work_item', ref: item.id, ageMs: now - base })
  }
  for (const card of engineClaims) {
    const base = parseDbTime(card.updatedAt) ?? parseDbTime(card.at)
    if (base !== null) {
      claims.push({ ledger: 'forge_engine_task_execution', ref: card.storyId, ageMs: now - base })
    }
  }
  const oldestClaim = claims.length
    ? claims.reduce((oldest, claim) => (claim.ageMs > oldest.ageMs ? claim : oldest))
    : null

  return {
    instances: runCards.length,
    openTasks: openRunCards.length,
    openWorkItems: queuedCards.length,
    activeClaims,
    oldestClaim,
  }
}

/** `<iso> <message>` per line; the timestamp is the first whitespace-delimited token. */
function parseInvocationLine(line: string): { at: string | null; message: string } {
  const match = /^(\S+)\s+(.*)$/.exec(line)
  if (match && /^\d{4}-\d{2}-\d{2}T/.test(match[1])) return { at: match[1], message: match[2] }
  return { at: null, message: line }
}

/**
 * Worker liveness from the scheduled worker's OWN invocation log.
 *
 * The free-text log is read defensively: a missing or empty log yields
 * UNKNOWN/never — never a crash and never a false healthy (the worker was dead
 * for twelve days while the board looked healthy and empty).
 */
function readWorkerLiveness(): DoctorWorkerLiveness {
  const logDir = process.env.AGENT_WORKER_LOG_DIR || join(homedir(), 'Library', 'Logs', 'CulebraLuxe')
  const logPath = join(logDir, 'agent-worker.invocations.log')
  if (!existsSync(logPath)) {
    return { status: 'unknown', newestInvocationAt: null, lastFailure: null, invocations: null, logPath }
  }

  const lines = readFileSync(logPath, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
  if (lines.length === 0) {
    return { status: 'unknown', newestInvocationAt: null, lastFailure: null, invocations: 0, logPath }
  }

  const parsed = lines.map(parseInvocationLine)
  const invocations = parsed.filter((entry) => /^start:/.test(entry.message)).length

  let status: DoctorWorkerLiveness['status'] = 'alive'
  let lastFailure: string | null = null
  for (let i = parsed.length - 1; i >= 0; i -= 1) {
    const message = parsed[i].message
    const exit = /exit=(\d+)/.exec(message)
    if (exit) {
      if (exit[1] !== '0') {
        status = 'failing'
        lastFailure = message
      }
      break
    }
    if (/^stop:/.test(message) && !/max passes reached/i.test(message)) {
      status = 'failing'
      lastFailure = message
      break
    }
    if (/^idle:/.test(message)) break
  }

  return { status, newestInvocationAt: parsed[parsed.length - 1].at, lastFailure, invocations, logPath }
}

async function gatherPostcard(): Promise<DoctorPostcard> {
  const [staging, stories, decisions, attempts] = await Promise.all([
    getStagingBatch(),
    listStoryboardStories(),
    listActiveDecisions('forge', { limit: 20 }),
    listRoiAttempts(ROI_DEFAULT_WINDOW_DAYS),
  ])

  const boardCount = (stories ?? []).filter((story) => story.status === 'Batched').length
  const tableCount = staging?.storyCount ?? 0
  const summary = summarizeRoi(attempts, ROI_DEFAULT_WINDOW_DAYS)
  const anchor = readLearnAnchor(process.cwd())

  return {
    boardCount,
    tableCount,
    activeDecisions: decisions.length,
    roiWindowDays: ROI_DEFAULT_WINDOW_DAYS,
    roiRows: summary.rows.map((row) => describeRoiRow(row)),
    newestLearnPassAt: anchor?.at ?? null,
  }
}

type QaConsistencyRow = { storyId: string; runId: string; result: QaConsistencyResult }

/**
 * QA run/verdict agreement across every story with a QA-lane run.
 *
 * READ-ONLY: it issues no insert, no update and no claim. `listStoryboardRuns`
 * is newest-first, so the FIRST QA run seen for a story is its latest. A story
 * with no QA run, or a QA run with no durable verdict, reports `unknown` — never
 * a false `agree`.
 */
async function gatherQaConsistency(): Promise<string> {
  let runs: StoryRun[] | null = null
  try {
    runs = await listStoryboardRuns()
  } catch {
    runs = null
  }
  if (!runs || runs.length === 0) {
    return 'QA CONSISTENCY\n  (no storyboard runs available)'
  }

  const latestQaRunByStory = new Map<string, StoryRun>()
  for (const run of runs) {
    if (!isQaRunType(run.runType)) continue
    if (!latestQaRunByStory.has(run.storyId)) latestQaRunByStory.set(run.storyId, run)
  }

  const rows: QaConsistencyRow[] = []
  for (const [storyId, run] of latestQaRunByStory) {
    let verdict: boolean | null = null
    try {
      const evidence = await readForgeWorkflowEvidence(storyId)
      verdict = evidence.qaPassed ?? null
    } catch {
      verdict = null
    }
    rows.push({
      storyId,
      runId: run.id,
      result: checkQaRunVerdictConsistency({ runStatus: run.resultStatus, verdict }),
    })
  }

  const agree = rows.filter((row) => row.result.state === 'agree').length
  const unknown = rows.filter((row) => row.result.state === 'unknown').length
  const disagree = rows.filter((row) => row.result.state === 'disagree')

  const lines = ['QA CONSISTENCY']
  lines.push(
    `  qa runs checked: ${rows.length} (agree ${agree} / unknown ${unknown} / DISAGREE ${disagree.length})`,
  )
  if (rows.length === 0) lines.push('  (no QA lane runs recorded)')
  for (const row of disagree) {
    lines.push(`  ${row.storyId}: ${renderQaConsistencyLine(row.result)}`)
  }
  return lines.join('\n')
}

async function main(): Promise<void> {
  const [controlPlane, postcard, qaConsistency] = await Promise.all([
    gatherControlPlane(),
    gatherPostcard(),
    gatherQaConsistency(),
  ])
  const worker = readWorkerLiveness()
  console.log(
    renderForgeDoctorReport({ controlPlane, worker, postcard, now: new Date().toISOString() }),
  )
  console.log('')
  console.log(qaConsistency)
}

void main()
