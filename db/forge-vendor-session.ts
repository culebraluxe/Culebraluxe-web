import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * The vendor session pointer for a story and its lane.
 *
 * A session id is a POINTER and a valuable one: one session serves a whole generation, so the next role
 * resumes the same context instead of rebuilding it — real token money. It used to live in the worktree file
 * `.forge-session.continue`, which made the lane the keeper of the fact and forced every lane to own a tree.
 * It lives in `forge_vendor_session` now, so the engine can ask for it and it is auditable in a query.
 *
 * `null` means either "nothing recorded yet" or "the recorded session is dead and was cleared" — both are
 * honestly "start fresh", which is why one null covers them.
 */
export async function readVendorSessionId(
  storyId: string,
  lane: string,
  execute?: QueryExecutor,
): Promise<string | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select session_id from forge_vendor_session
    where story_id = ${storyId} and worker_id = ${lane}
    limit 1
  `
  const value = rows[0]?.session_id
  return typeof value === 'string' && value.trim() ? value : null
}

/**
 * Record the session this generation is running in, or CLEAR it (`null`) when the session died.
 *
 * Clearing is the "a dead session is replaced once" rule: the role that discovers a dead session drops the
 * pointer, so the next role starts a fresh session and records its new id. Without it the pointer would keep
 * naming a corpse and every later role would fail in turn.
 */
export async function writeVendorSessionId(
  storyId: string,
  lane: string,
  sessionId: string | null,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    insert into forge_vendor_session (story_id, worker_id, session_id, updated_at)
    values (${storyId}, ${lane}, ${sessionId}, now())
    on conflict (story_id, worker_id)
    do update set session_id = excluded.session_id, updated_at = now()
  `
}
