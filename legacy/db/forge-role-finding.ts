/**
 * FINDINGS IN ROWS (migration 172) — the last contract that travelled as chat JSON.
 *
 * Architect and Scout still emit `FORGE_ARCHITECT_HANDOFF` / `FORGE_FINDINGS_JSON` inside
 * the reply, so the only way to learn what they saw is a brace-balanced scan of chat. A
 * dropped marker costs a run; a stray brace inside a quoted shell command truncates a
 * plan. `scripts/forge-handoff.mjs --finding` writes these rows instead, and the database
 * refuses exactly what the architect gate refuses.
 *
 * AUTHORITY ORDER: rows win, the reply parser is the fallback — and `null` here means
 * "no rows were written", which is the honest absence of a snapshot. It is NOT an empty
 * plan, so a caller can tell "the Architect recorded nothing" from "the Architect
 * recorded findings that turned out to be empty".
 *
 * Cross-node reads are by STORY, because the consumer that matters (the Lead's routing
 * context) runs in a different task from the Architect that wrote the rows.
 */
import type { QueryExecutor } from '@/legacy/db/query-executor'
import type { ArchitectFinding } from '@/legacy/workflow_app/forge/forge-shaping'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('@/legacy/db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** Map a finding row to the live ArchitectFinding the gates already understand. */
function toFinding(row: Record<string, unknown>): ArchitectFinding {
  const hint = row.hint == null ? null : (String(row.hint) as ArchitectFinding['hint'])
  return {
    id: String(row.finding_id),
    summary: String(row.summary),
    required: row.required === true,
    seams: Array.isArray(row.seams) ? (row.seams as string[]) : [],
    ...(hint ? { hint } : {}),
  }
}

/**
 * The findings recorded for one task/node/attempt, or null when none were written.
 * Null means "fall back to the reply parser and record channel=legacy_json".
 */
export async function getForgeRoleFindings(
  key: { taskId: string; nodeId: string; attempt: number },
  execute?: QueryExecutor,
): Promise<ArchitectFinding[] | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select finding_id, summary, required, seams, hint
    from forge_role_finding
    where task_id = ${key.taskId} and node_id = ${key.nodeId} and attempt = ${key.attempt}
    order by finding_id
  `
  if (rows.length === 0) return null
  return rows.map((row) => toFinding(row as Record<string, unknown>))
}

/**
 * The findings recorded for the CURRENT RUN of a story: newest attempt of each node,
 * scoped to one process instance.
 *
 * This is what the Lead's routing context reads — the Architect's rows were written under
 * the Architect's task, so the join is by story. But the scope is the PROCESS INSTANCE and
 * the NEWEST ATTEMPT, and both halves are load-bearing:
 *
 *   - Without the process scope, every previous run of the same story contributes rows, so
 *     a story that has ever been re-run reads as one handoff with each finding repeated.
 *   - Without the attempt scope, an Architect that retried (writing the same finding ids
 *     under attempt 2, which the unique index permits by design) reads as duplicates.
 *   - Without the TASK scope, an operator re-run that creates a SECOND task for the same node
 *     (both at attempt 1, which the attempt rule cannot separate) contributes both tasks' rows,
 *     so the same finding id arrives twice and the gate refuses every route. Observed live on
 *     2026-09-17: two architect tasks wrote 4 and 5 rows for 5 ids, and the only remedy was an
 *     operator deleting PROD rows by task id.
 *
 * Either way the Lead's gate rejects the context with "Duplicate finding IDs in Architect
 * handoff" and the story can never route — observed live on 2026-09-13, and the reason the
 * chain could not complete a single story. A vague read is not a smaller read; it is a
 * wrong one. The newest task per node is chosen by max `created_at` with `task_id` as a
 * deterministic tie-break, so a same-millisecond re-run cannot pick at random.
 *
 * Pass `null` findings when nothing was written: the caller distinguishes that from an
 * empty handoff.
 */
export async function listStoryForgeFindings(
  key: { storyId: string; processInstanceId: string },
  execute?: QueryExecutor,
): Promise<ArchitectFinding[] | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    with per_task as (
      select node_id, task_id, max(created_at) as last_written
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
      group by node_id, task_id
    ),
    newest_task as (
      select node_id, task_id
      from (
        select node_id, task_id,
               row_number() over (partition by node_id order by last_written desc, task_id desc) as rn
        from per_task
      ) ranked
      where rn = 1
    ),
    latest as (
      select node_id, max(attempt) as attempt
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
        and task_id in (select task_id from newest_task)
      group by node_id
    )
    select f.finding_id, f.summary, f.required, f.seams, f.hint
    from forge_role_finding f
    join newest_task t on t.node_id = f.node_id and t.task_id = f.task_id
    join latest l on l.node_id = f.node_id and l.attempt = f.attempt
    where f.story_id = ${key.storyId} and f.process_instance_id = ${key.processInstanceId}
    order by f.finding_id
  `
  if (rows.length === 0) return null
  return rows.map((row) => toFinding(row as Record<string, unknown>))
}

/** One finding's declared seams, as the write guard reads them. */
export type FindingSeamSet = { findingId: string; seams: string[] }

/** A seam a later attempt dropped, with the finding that declared it in the prior attempt. */
export type FindingSupersede = {
  seam: string
  declaredByAttempt: number
  declaredByFindingId: string
}

export type FindingWriteDecision =
  | { kind: 'allow'; superseded: FindingSupersede[] }
  | { kind: 'refuse'; dropped: FindingSupersede[] }

/**
 * THE ONE DECISION A FINDINGS WRITE MAKES ABOUT SEAMS IT WOULD DROP.
 *
 * A re-issued findings set that loses a seam the previous attempt declared used to be a
 * silent drop: the write upserted one row per finding, never looked at attempt N-1, and the
 * reader's newest-attempt scope then hid the loss from the Lead. This rule makes the drop
 * either REFUSED (named) or SUPERSEDED (a typed row), never silent.
 *
 * The unit is the SEAM, not the finding id: an attempt may re-key its findings freely, and a
 * finding that only repeats or adds seams is allowed. `dropped` is every seam attempt N-1
 * declared that the attempt in progress (its rows so far plus this one) no longer declares.
 *
 * `acknowledged` is the explicit `--supersede` set. A non-empty `dropped` with no exact
 * acknowledgement is refused; an acknowledgement that is a subset or a superset is refused
 * too, because either one would launder an unacknowledged drop.
 */
export function decideFindingWrite(input: {
  attempt: number
  prior: { attempt: number; findings: FindingSeamSet[] } | null
  current: FindingSeamSet[]
  incoming: FindingSeamSet
  acknowledged: string[]
}): FindingWriteDecision {
  const declaredBy = new Map<string, string>()
  for (const finding of input.prior?.findings ?? []) {
    for (const seam of finding.seams) {
      if (!declaredBy.has(seam)) declaredBy.set(seam, finding.findingId)
    }
  }
  const kept = new Set<string>()
  for (const finding of [...input.current, input.incoming]) {
    for (const seam of finding.seams) kept.add(seam)
  }
  const declaredByAttempt = input.prior?.attempt ?? input.attempt - 1
  const dropped: FindingSupersede[] = [...declaredBy.entries()]
    .filter(([seam]) => !kept.has(seam))
    .map(([seam, declaredByFindingId]) => ({ seam, declaredByAttempt, declaredByFindingId }))
  if (dropped.length === 0) return { kind: 'allow', superseded: [] }
  const acknowledged = new Set(input.acknowledged)
  const exact =
    acknowledged.size === dropped.length && dropped.every((entry) => acknowledged.has(entry.seam))
  return exact ? { kind: 'allow', superseded: dropped } : { kind: 'refuse', dropped }
}

/** A task whose findings a later task for the SAME node superseded, and the task that replaced it. */
export type FindingRetiredTask = {
  nodeId: string
  taskId: string
  supersededByTaskId: string
}

/** The newest attempt in force for a story run, its findings, and the seams a later attempt superseded. */
export type ForgeFindingHandoff = {
  attemptInForce: number | null
  findings: ArchitectFinding[]
  superseded: FindingSupersede[]
  /**
   * Tasks a re-run retired for a node. The reader scopes findings to the newest TASK per node, so an
   * operator re-run leaves one set in force; this names the task that set replaced, and why. Empty when
   * a node has only ever had one task.
   */
  retiredTasks: FindingRetiredTask[]
}

/**
 * THE LEAD'S FINDINGS INPUT, WITH THE ATTEMPT THAT IS IN FORCE AND WHAT IT LOST.
 *
 * Same scope as `listStoryForgeFindings` — this process instance, the newest attempt per node
 * — plus two facts the Lead could not previously see: which attempt is in force, and every
 * seam a later attempt explicitly superseded (recorded in `forge_role_finding_supersede`).
 *
 * `null` means no finding rows were written, so the caller falls back to the reply parser and
 * must state no attempt rather than a fabricated one. A row whose attempt is missing is read
 * as no attempt, never as 0 or 1.
 */
export async function listStoryForgeFindingHandoff(
  key: { storyId: string; processInstanceId: string },
  execute?: QueryExecutor,
): Promise<ForgeFindingHandoff | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    with per_task as (
      select node_id, task_id, max(created_at) as last_written
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
      group by node_id, task_id
    ),
    newest_task as (
      select node_id, task_id
      from (
        select node_id, task_id,
               row_number() over (partition by node_id order by last_written desc, task_id desc) as rn
        from per_task
      ) ranked
      where rn = 1
    ),
    latest as (
      select node_id, max(attempt) as attempt
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
        and task_id in (select task_id from newest_task)
      group by node_id
    )
    select f.finding_id, f.summary, f.required, f.seams, f.hint, f.attempt
    from forge_role_finding f
    join newest_task t on t.node_id = f.node_id and t.task_id = f.task_id
    join latest l on l.node_id = f.node_id and l.attempt = f.attempt
    where f.story_id = ${key.storyId} and f.process_instance_id = ${key.processInstanceId}
    order by f.finding_id
  `
  if (rows.length === 0) return null
  const findings = rows.map((row) => toFinding(row as Record<string, unknown>))
  let attemptInForce = 0
  for (const row of rows) {
    const value = Number((row as Record<string, unknown>).attempt)
    if (Number.isFinite(value) && value > attemptInForce) attemptInForce = value
  }
  const supersededRows = await q`
    select seam, declared_by_attempt, declared_by_finding_id
    from forge_role_finding_supersede
    where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
    order by seam
  `
  const superseded: FindingSupersede[] = supersededRows.map((row) => {
    const record = row as Record<string, unknown>
    return {
      seam: String(record.seam),
      declaredByAttempt: Number(record.declared_by_attempt),
      declaredByFindingId: String(record.declared_by_finding_id),
    }
  })
  // WHICH TASK THE RE-RUN RETIRED, AND WHY. The findings read above keeps only the newest task per
  // node; every OTHER task that wrote findings for that node is a task a re-run superseded. This is
  // the operator-legible record of the supersede — the same scope rule, read back, so a manual row
  // deletion is never the only remedy. A row missing any of the three facts is skipped rather than
  // rendered as "undefined".
  const retiredRows = await q`
    with per_task as (
      select node_id, task_id, max(created_at) as last_written
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
      group by node_id, task_id
    ),
    ranked as (
      select node_id, task_id,
             row_number() over (partition by node_id order by last_written desc, task_id desc) as rn
      from per_task
    )
    select r.node_id, r.task_id, w.task_id as superseded_by_task_id
    from ranked r
    join ranked w on w.node_id = r.node_id and w.rn = 1
    where r.rn > 1
    order by r.node_id, r.task_id
  `
  const retiredTasks: FindingRetiredTask[] = []
  for (const row of retiredRows) {
    const record = row as Record<string, unknown>
    const nodeId = String(record.node_id ?? '')
    const taskId = String(record.task_id ?? '')
    const supersededByTaskId = String(record.superseded_by_task_id ?? '')
    if (!nodeId || !taskId || !supersededByTaskId || taskId === supersededByTaskId) continue
    retiredTasks.push({ nodeId, taskId, supersededByTaskId })
  }
  return {
    attemptInForce: attemptInForce > 0 ? attemptInForce : null,
    findings,
    superseded,
    retiredTasks,
  }
}
