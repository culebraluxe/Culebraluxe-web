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
import type { QueryExecutor } from './query-executor'
import type { ArchitectFinding } from '../workflow_app/forge/forge-shaping'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
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
 *
 * Either way the Lead's gate rejects the context with "Duplicate finding IDs in Architect
 * handoff" and the story can never route — observed live on 2026-09-13, and the reason the
 * chain could not complete a single story. A vague read is not a smaller read; it is a
 * wrong one.
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
    with latest as (
      select node_id, max(attempt) as attempt
      from forge_role_finding
      where story_id = ${key.storyId} and process_instance_id = ${key.processInstanceId}
      group by node_id
    )
    select f.finding_id, f.summary, f.required, f.seams, f.hint
    from forge_role_finding f
    join latest l on l.node_id = f.node_id and l.attempt = f.attempt
    where f.story_id = ${key.storyId} and f.process_instance_id = ${key.processInstanceId}
    order by f.finding_id
  `
  if (rows.length === 0) return null
  return rows.map((row) => toFinding(row as Record<string, unknown>))
}
