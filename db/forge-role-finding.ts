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
 * The findings recorded for a STORY, newest attempt first per node, or null.
 * This is what the Lead's routing context reads: the Architect's rows were written under
 * the Architect's task, not the Lead's.
 */
export async function listStoryForgeFindings(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ArchitectFinding[] | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select finding_id, summary, required, seams, hint
    from forge_role_finding
    where story_id = ${storyId}
    order by created_at, finding_id
  `
  if (rows.length === 0) return null
  return rows.map((row) => toFinding(row as Record<string, unknown>))
}
