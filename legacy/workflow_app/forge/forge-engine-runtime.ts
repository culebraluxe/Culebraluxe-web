import { spawnSync } from 'node:child_process'

function rustForgeTask(args: string[]): string {
  const bin = process.env.FORGE_TASK_BIN?.trim()
  if (!bin) {
    throw new Error(
      'FORGE_TASK_BIN is not set. Build rust/forge-task and export the path. cargo run per task is refused.',
    )
  }
  const r = spawnSync(bin, args, { encoding: 'utf8', env: process.env })
  if (r.status !== 0) {
    throw new Error((r.stderr || r.stdout || `forge-task ${args.join(' ')} failed`).trim())
  }
  return (r.stdout || '').trim()
}

import { FORGE_SDLC_KEY } from '@/legacy/workflow_app/definitions/forge-sdlc'
import { engineConfigured, engineSql } from '@/legacy/workflow_app/engine-client'
import { startWorkflowCore } from '@/legacy/workflow_app/start-core'
import { captureServerError } from '@/lib/server-error-capture'
import type { QueryExecutor } from '@/legacy/db/query-executor'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'



// ---------------------------------------------------------------------------
// ENG-FORGE-V10 — Forge engine runtime.
//
// Mirrors the RE start boundary (workflow_app/runtime.ts + start-core.ts): a
// story becomes a running FORGE_SDLC instance on the SAME shared engine DB via
// the production interactive client (engineSql -> lib/neon-interactive WebSocket
// Pool routed by resolveDbTarget, so APP_ENV=production -> PROD, else DEV).
//
//   story ready
//     -> locate active FORGE_SDLC instance for the story (idempotent)
//     -> start instance (subject_type='story', subject_id=storyId,
//        variables.workType) so classify_work routes; the instance then parks
//        at the first async role task-node.
//
// Async roles are task-nodes: when the instance reaches one, the Forge
// execution layer runs the role agent and completes the engine task via
// engine.completeTask (see listActiveForgeRoleTasks / the Forge execution
// seam). Synchronous DEV_OPS command-nodes route through the Forge
// ApplicationPort.
// ---------------------------------------------------------------------------

export async function findActiveForgeInstance(
  storyId: string,
): Promise<string | null> {
  if (!engineConfigured()) return null
  const rows = await engineSql()`
    select pi.id
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'story'
      and pi.subject_id = ${storyId}
      and pi.status = 'active'
      and pd.key = ${FORGE_SDLC_KEY}
    limit 1
  `
  return (rows[0]?.id as string | undefined) ?? null
}

export type ForgeStartFacts = {
  workType: 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION' | 'FAST'
  /** Optional initial gate evidence (e.g. rootCauseKnown for BUG). */
  evidence?: ForgeGateEvidence
}

/**
 * Idempotently start a FORGE_SDLC engine instance for a story. Returns the
 * active instance id and whether a new instance was started.
 */
export async function startForgeWorkflow(
  storyId: string,
  input: ForgeStartFacts,
): Promise<{ instanceId: string; started: boolean }> {
  return startWorkflowCore(storyId, {
    findActive: (id) => findActiveForgeInstance(id),
    readFacts: async () => ({ ...(input.evidence ?? {}) }),
    start: async (id, _facts) => {
      if (!engineConfigured()) {
        throw new Error('Workflow engine database is not configured.')
      }
      // Initial evidence is not durable until startProcess returns its instance
      // id. Keep that exact input visible while the synchronous start traversal
      // evaluates entry decisions; immediately persist it after creation.
      const processInstanceId = rustForgeTask([
        'start',
        '--story',
        id,
        '--work-type',
        input.workType,
      ])
      const { mergeForgeWorkflowEvidence } = await import('@/legacy/db/forge-workflow-evidence')
      await mergeForgeWorkflowEvidence(processInstanceId, id, {
        workType: input.workType,
        ...(input.evidence ?? {}),
      })
      const { markForgeStoryInProgress } = await import('@/legacy/db/forge-story-state')
      await markForgeStoryInProgress(id)
      // V1 estimator: seed one idempotent forecast per story so real (coarse)
      // Rows start accumulating for later calibration. Never blocks story start,
      // but a failure is captured durably (WARN) — it is real, not a silent catch.
      try {
        await (await import('@/legacy/db/forge-estimator')).seedForecastForStory(id)
      } catch (error) {
        captureServerError('forge:seed-forecast', error, { level: 'warn' })
      }
      return processInstanceId
    },
  })
}

export type ActiveForgeRoleTask = {
  taskId: string
  processInstanceId: string
  tokenId: string
  nodeId: string
  status: string
  assignee: string | null
  candidates: string[]
  formData: Record<string, any>
}

/**
 * List the open (ready/reserved/in_progress) engine role task-nodes for a
 * story's active FORGE_SDLC instance — the async gates the Forge execution
 * layer must run and complete via engine.completeTask.
 */
export async function listActiveForgeRoleTasks(
  storyId: string,
): Promise<ActiveForgeRoleTask[]> {
  const instanceId = await findActiveForgeInstance(storyId)
  if (!instanceId) return []
  const rows = await engineSql()`
    select t.id as task_id, t.process_instance_id, t.token_id,
      tk.node_id as node_id, t.status, t.assignee, t.candidates, t.form_data
    from tasks t
    join tokens tk on tk.id = t.token_id
    where t.process_instance_id = ${instanceId}
      and t.status in ('ready', 'reserved', 'in_progress')
    order by t.created_at, t.id
  `
  return (rows as Array<{
    task_id: string
    process_instance_id: string
    token_id: string
    node_id: string
    status: string
    assignee: string | null
    candidates: string[]
    form_data: Record<string, any> | null
  }>).map((r) => ({
    taskId: r.task_id,
    processInstanceId: r.process_instance_id,
    tokenId: r.token_id,
    nodeId: r.node_id,
    status: r.status,
    assignee: r.assignee ?? null,
    candidates: r.candidates ?? [],
    formData: r.form_data ?? {},
  }))
}

/**
 * THE STOP POINT OF A RUN — the newest OPEN engine task on an instance, at ANY node.
 *
 * `listActiveForgeRoleTasks` answers per story; this answers per instance for the resume
 * door. It is the seam that lets the door reach a run that errored at a lane and never
 * reached the hold gate: a run stopped at `smith` has a task at `smith`, not at `hold`,
 * so `resolveForgeHold`'s old hold-task-only lookup could not touch it.
 */
export async function findOpenForgeTask(
  instanceId: string,
): Promise<{
  taskId: string
  nodeId: string
  claimedAt: string | null
  forkChild: boolean
  openSiblings: number
} | null> {
  if (!engineConfigured()) return null
  // WHY THIS CARRIES MORE THAN AN ID (ENG-FORGE-SPLIT-SIBLING-01): the door used to take
  // "the newest open task" and advance it, which is right for a lane parked mid-work and
  // WRONG for a dynamic-fork branch that has never been claimed — advancing that one marks
  // a sibling's work done without running it. Observed 2026-09-18: a resume at 09:34
  // completed branch 0 of 2 (claimed_at null, completed_by operator), so the sibling's work
  // item never existed, the join could never be satisfied, and every later run could only
  // reach branch 1. The caller now refuses instead of fabricating that completion.
  const rows = await engineSql()`
    with open_tasks as (
      select t.id as task_id,
             tk.node_id as node_id,
             t.claimed_at,
             tk.parent_token_id as fork_parent
      from tasks t
      join tokens tk on tk.id = t.token_id
      where t.process_instance_id = ${instanceId}
        and t.status in ('ready', 'reserved', 'in_progress')
    )
    select o.task_id,
           o.node_id,
           o.claimed_at,
           (o.fork_parent is not null) as fork_child,
           (
             select count(*)
             from open_tasks s
             where s.fork_parent is not null
               and s.fork_parent = o.fork_parent
               and s.task_id <> o.task_id
           ) as open_siblings
    from open_tasks o
    order by o.task_id desc
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return {
    taskId: String(row.task_id),
    nodeId: String(row.node_id),
    claimedAt: row.claimed_at ? new Date(row.claimed_at as string).toISOString() : null,
    forkChild: Boolean(row.fork_child),
    openSiblings: Number(row.open_siblings ?? 0),
  }
}


export { FORGE_SDLC_KEY as FORGE_DEFINITION_KEY }

/**
 * Complete an async role task-node on a Forge engine instance (the async
 * resume the task-node model enables). The Forge execution layer runs the role
 * agent, then calls this to advance the token on the chosen transition. Any
 * decision-gate evidence is supplied via `evidence` so downstream gates route;
 * when none is supplied the engine falls back to each decision's first
 * transition (e.g. SOLO for execution_shape).
 */
export async function completeForgeRoleTask(
  taskId: string,
  opts: {
    transitionName?: string
    evidence?: ForgeGateEvidence
    userId?: string
    /** Test seam for the completion unit (crash injection). Never set in production. */
    unit?: ForgeCompletionUnitDeps
  } = {},
): Promise<void> {
  if (!engineConfigured()) {
    throw new Error('Workflow engine database is not configured.')
  }
  const evidence = opts.evidence ?? {}
  const taskRows = await engineSql()`
    select t.process_instance_id, pi.subject_id as story_id, tk.node_id as node_id
    from tasks t
    join process_instances pi on pi.id = t.process_instance_id
    left join tokens tk on tk.id = t.token_id
    where t.id = ${taskId}
  `
  const task = taskRows[0]
  if (!task?.process_instance_id || !task?.story_id) {
    throw new Error(`Forge task ${taskId} is missing its workflow/story relationship`)
  }
  // Transition is the Rust engine CAS. TS only writes evidence after a win.

  // THE TRANSITION GOES FIRST — IT IS THE CAS THAT DECIDES THE WINNER.
  //
  // This order is the fix for a real race: merging evidence BEFORE the transition let a
  // worker that LOST the race commit its result anyway. The engine said worker A won
  // while the durable row held worker B's findings, candidate SHA and QA state — the one
  // place the two systems could disagree, and they disagreed silently.
  //
  // Now nothing is committed unless the transition actually won. A losing advance
  // throws out of completeTask, so nothing below runs and the loser writes nothing.
  //
  // The QA-failure ledger write in forge-executor.ts deliberately STAYS before this
  // call: the engine's qa_failure_route reads the durable disposition WHILE the
  // transition runs, so that order is load-bearing, not accidental. Residual, stated
  // plainly: a losing QA worker can still record a disposition. Closing that needs one
  // shared transaction across the engine and the ledger writers, which is a larger
  // change than this one.
  rustForgeTask([
    'complete',
    '--task',
    taskId,
    '--user',
    opts.userId ?? 'forge',
    '--transition',
    opts.transitionName ?? 'complete',
  ])

  // Only a worker that WON the transition reaches here. The evidence merge and any
  // repair/replan increment now commit as ONE exactly-once unit behind a claim-first
  // receipt (see applyForgeCompletionUnit): if either write fails the transaction rolls
  // back and the receipt is not committed, so a later run FINISHES the unit from the
  // engine's own durable record instead of re-running the role. A crash between the
  // transition and the unit is the same story — reconcileForgeCompletions picks it up.
  await applyForgeCompletionUnit(
    {
      taskId,
      processInstanceId: task.process_instance_id as string,
      storyId: task.story_id as string,
      nodeId: task.node_id == null ? null : String(task.node_id),
      evidence,
    },
    opts.unit,
  )
}

// ---------------------------------------------------------------------------
// THE COMPLETION UNIT — advancing the engine, persisting evidence and counting a
// repair are one recoverable, idempotent step.
//
// The engine's transition is the CAS that decides the winner and is already
// exactly-once (ENG-13). What was NOT recoverable was everything AFTER it: the
// evidence merge and the repair/replan increment were separate writes, so a crash
// between them left an advanced workflow with no result and an undercounted repair.
//
// One claim-first receipt keyed by task id serializes the unit: the claim, the
// evidence merge, the counter increment and the finalize all run in ONE transaction,
// so a crash rolls back the whole unit and the receipt is absent — which is exactly
// the durable signal a later run reconciles on. The 'pending' sentinel therefore never
// persists (matching the receipt table's contract); the absence of a receipt IS the
// crash-window record.
// ---------------------------------------------------------------------------

export type ForgeCompletionUnitDeps = {
  mergeEvidence?: (
    processInstanceId: string,
    storyId: string,
    evidence: ForgeGateEvidence,
    execute?: QueryExecutor,
  ) => Promise<void>
  incrementRepair?: (storyId: string, execute: QueryExecutor) => Promise<unknown>
  incrementReplan?: (storyId: string, execute: QueryExecutor) => Promise<unknown>
  claim?: (tx: QueryExecutor, commandId: string) => Promise<boolean>
  finalize?: (tx: QueryExecutor, commandId: string) => Promise<void>
  /** Test seam: runs after the engine transition and before the unit — the first crash window. */
  afterTransition?: () => Promise<void> | void
  /** Test seam: runs after the evidence merge and before the counter — the second crash window. */
  afterEvidence?: () => Promise<void> | void
}

/** The receipt id that marks one engine task's completion as accounted. */
export function forgeCompletionReceiptId(taskId: string): string {
  return `forge.completion:${taskId}`
}

type ForgeInteractiveExecutor = QueryExecutor & {
  begin: (cb: (tx: QueryExecutor) => Promise<unknown>) => Promise<unknown>
}

/**
 * Apply one completed role task's effects exactly once: merge its evidence and, for a
 * repair node, count the attempt — all inside one claim-first receipt transaction.
 * A loser (or a re-run after a committed unit) claims nothing and returns.
 */
export async function applyForgeCompletionUnit(
  input: {
    taskId: string
    processInstanceId: string
    storyId: string
    nodeId: string | null
    evidence: ForgeGateEvidence
  },
  deps: ForgeCompletionUnitDeps = {},
): Promise<void> {
  await deps.afterTransition?.()
  const { mergeForgeWorkflowEvidence } = await import('@/legacy/db/forge-workflow-evidence')
  const { claimReceipt, finalizeReceipt } = await import('@/legacy/db/workflow-command-receipt')
  const { incrementForgeRepair, incrementForgeReplan } = await import('@/legacy/db/forge-repair-ledger')
  const mergeEvidence = deps.mergeEvidence ?? mergeForgeWorkflowEvidence
  const claim = deps.claim ?? ((tx: QueryExecutor, id: string) => claimReceipt(tx, id))
  const finalize =
    deps.finalize ??
    ((tx: QueryExecutor, id: string) => finalizeReceipt(tx, id, 'success', input.taskId, null))
  const incRepair =
    deps.incrementRepair ??
    ((storyId: string, tx: QueryExecutor) => incrementForgeRepair(storyId, tx))
  const incReplan =
    deps.incrementReplan ??
    ((storyId: string, tx: QueryExecutor) => incrementForgeReplan(storyId, tx))
  const receiptId = forgeCompletionReceiptId(input.taskId)
  const tx = engineSql() as ForgeInteractiveExecutor
  await tx.begin(async (client) => {
    const won = await claim(client, receiptId)
    if (!won) return
    await mergeEvidence(input.processInstanceId, input.storyId, input.evidence, client)
    await deps.afterEvidence?.()
    if (input.nodeId === 'repair_smith') await incRepair(input.storyId, client)
    else if (input.nodeId === 'repair_architect') await incReplan(input.storyId, client)
    await finalize(client, receiptId)
  })
}

/**
 * RESUME DOOR — finish any completed role task whose completion unit did not commit.
 *
 * Reads the evidence from the engine's own durable record (the `task.completed`
 * event the winning transition wrote) and applies the unit. A task whose receipt
 * already exists is skipped, so a later run never re-runs the role and never
 * double-counts a repair attempt. The watermark bounds the replay to completions
 * newer than the newest receipt, so history is not re-counted when the unit ships.
 */
export async function reconcileForgeCompletions(storyId: string): Promise<number> {
  if (!engineConfigured()) return 0
  const instanceId = await findActiveForgeInstance(storyId)
  if (!instanceId) return 0
  const { readFinalReceipt, readReceiptWatermark } = await import('@/legacy/db/workflow-command-receipt')
  const watermark = await readReceiptWatermark(engineSql(), 'forge.completion:')
  const rows = await engineSql()`
    select e.task_id, tk.node_id, e.data->'formData' as form_data
    from process_events e
    join tasks t on t.id = e.task_id
    left join tokens tk on tk.id = t.token_id
    where e.process_instance_id = ${instanceId}
      and e.event_type = 'task.completed'
      and t.status = 'completed'
      and (${watermark}::timestamptz is null or e.created_at > ${watermark}::timestamptz)
    order by e.created_at, e.id
  `
  let applied = 0
  for (const row of rows) {
    const taskId = String(row.task_id)
    const existing = await readFinalReceipt(engineSql(), forgeCompletionReceiptId(taskId))
    if (existing) continue
    await applyForgeCompletionUnit({
      taskId,
      processInstanceId: instanceId,
      storyId,
      nodeId: row.node_id == null ? null : String(row.node_id),
      evidence: (row.form_data ?? {}) as ForgeGateEvidence,
    })
    applied += 1
  }
  return applied
}

/** Claim a Forge engine task before any external runner is launched. */
export async function claimForgeRoleTask(taskId: string, workerId: string): Promise<void> {
  if (!engineConfigured()) {
    throw new Error('Workflow engine database is not configured.')
  }
  rustForgeTask(['claim', '--task', taskId, '--user', workerId])
}

/** Release a claimed task after an interrupted/failed external launch. */
export async function releaseForgeRoleTask(taskId: string, workerId: string): Promise<void> {
  if (!engineConfigured()) {
    throw new Error('Workflow engine database is not configured.')
  }
  rustForgeTask(['release', '--task', taskId, '--user', workerId])
}

/**
 * Terminate a stopped instance as `cancelled` WITHOUT needing a hold task.
 *
 * This is the door's cancel path for a run that errored at a lane: the engine locks the
 * owning instance FIRST and serializes termination against every task mutation (ENG-11),
 * so an open lane task does not block it. A non-active instance throws — the caller
 * surfaces that as a named refusal rather than a silent success.
 */
export async function cancelForgeInstance(
  instanceId: string,
  opts: { actor: string; reason?: string },
): Promise<void> {
  if (!engineConfigured()) {
    throw new Error('Workflow engine database is not configured.')
  }
  const args = ['cancel', '--instance', instanceId]
  if (opts.reason) args.push('--reason', opts.reason)
  rustForgeTask(args)
}

export type ForgeStoryboardProjection =
  | { state: 'in_progress' }
  | { state: 'complete' }
  | { state: 'hold'; reason: string }
  | { state: 'failed'; reason: string }

/** Pure engine-terminal -> Storyboard projection used by recovery and tests. */
export function projectForgeStoryboardState(input: {
  status: string
  outcome: string | null
  humanHold?: boolean
}): ForgeStoryboardProjection {
  if (input.humanHold) {
    return { state: 'hold', reason: 'Forge engine entered a human decision gate.' }
  }
  if (input.status === 'completed' && input.outcome === 'completed') {
    return { state: 'complete' }
  }
  if (input.status === 'aborted' || input.outcome === 'cancelled') {
    return { state: 'hold', reason: 'Forge engine workflow was cancelled.' }
  }
  if (input.status === 'error' || input.status === 'completed') {
    return {
      state: 'failed',
      reason: `Forge engine terminated with outcome ${String(input.outcome ?? 'failed')}.`,
    }
  }
  return { state: 'in_progress' }
}

/** Reconcile the Storyboard projection from engine truth. Idempotent. */
export async function syncForgeStoryboardState(
  storyId: string,
  instanceId: string,
  options: { humanHold?: boolean } = {},
): Promise<void> {
  const rows = await engineSql()`
    select status, outcome
    from process_instances
    where id = ${instanceId}
  `
  const instance = rows[0]
  if (!instance) throw new Error(`Forge process instance ${instanceId} was not found`)
  const storyState = await import('@/legacy/db/forge-story-state')
  const projection = projectForgeStoryboardState({
    status: String(instance.status),
    outcome: instance.outcome == null ? null : String(instance.outcome),
    humanHold: options.humanHold,
  })
  switch (projection.state) {
    case 'complete': {
      // ENG-FORGE-MIGRATION-APPLIED-01 — engine-terminal `complete` is reconciled onto the
      // Storyboard HERE, so this is the seam that must refuse Complete while the PROD
      // migration ledger lacks a migration the story's change set adds. A story with no
      // migration in its change set, and one whose migration is ledgered, are unaffected.
      // An unverifiable ledger FAILS CLOSED to a HOLD — never to Complete.
      let holdReason: string | null
      try {
        holdReason = await migrationLedgerHoldReason(storyId)
      } catch (error) {
        holdReason =
          'Forge completion held: could not verify the PROD migration ledger for this story — ' +
          String((error as Error)?.message ?? error)
      }
      if (holdReason) {
        await storyState.markForgeStoryHumanHold(storyId, holdReason)
        break
      }
      await storyState.markForgeStoryPublishedComplete(storyId)
      break
    }
    case 'hold':
      await storyState.markForgeStoryHumanHold(storyId, projection.reason)
      break
    case 'failed':
      await storyState.markForgeStoryFailed(storyId, projection.reason)
      break
    case 'in_progress':
      await storyState.markForgeStoryInProgress(storyId)
      break
  }
}

/**
 * The PROD migration-ledger hold reason for a story — or null when its change set adds no
 * migration, or adds only migrations the ledger already carries.
 *
 * The change set is the same git answer the scope checks use: the story's own commits
 * (newest first, from `storyboard_story_run`), diffed from the parent of its earliest
 * commit. The ledger is read from the LEDGER TABLE (PROD `schema_migration`), never the
 * filesystem — a file on disk is not proof it was applied.
 */
async function migrationLedgerHoldReason(storyId: string): Promise<string | null> {
  const [{ listStoryCommitHashes }, { changedFilesForCandidate }, guard] = await Promise.all([
    import('@/legacy/db/storyboard'),
    import('@/lib/worker-workspace/candidate-diff'),
    import('@/legacy/workflow_app/forge/migration-applied-guard'),
  ])
  const commits = await listStoryCommitHashes(storyId)
  if (commits.length === 0) return null
  const changedPaths = await changedFilesForCandidate({
    cwd: process.cwd(),
    baseRef: `${commits[commits.length - 1]}^`,
    candidateSha: commits[0],
  })
  const assessment = await guard.guardMigrationApplied({ changedPaths })
  return assessment.ok ? null : guard.migrationAppliedRefusal(assessment.unapplied)
}
