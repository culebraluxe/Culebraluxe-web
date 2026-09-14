import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type LandedAppleReminder = {
  sourceAccount: string | null
  sourceMessageId: string
  externalId?: string | null
  listName?: string | null
  title?: string | null
  notes?: string | null
  startsAt?: string | null
  dueAt?: string | null
  completed: boolean
  completedAt?: string | null
  priority?: number | null
  raw: unknown
}

/**
 * Apple Reminders are mutable task state, unlike immutable message evidence.
 * Replay therefore refreshes due/completion/list/title state in-place while
 * preserving first_seen_at. This is still only landing/current-state evidence;
 * it never creates or mutates canonical WBS work.
 */
export async function upsertAppleReminder(
  input: LandedAppleReminder,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    insert into l_reminder (
      source_account, source_message_id, external_id, list_name, title, notes,
      starts_at, due_at, completed, completed_at, priority, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.externalId ?? null},
      ${input.listName ?? null}, ${input.title ?? null}, ${input.notes ?? null},
      ${input.startsAt ?? null}::timestamptz, ${input.dueAt ?? null}::timestamptz,
      ${input.completed}, ${input.completedAt ?? null}::timestamptz,
      ${input.priority ?? null}, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do update set
      external_id = excluded.external_id,
      list_name = excluded.list_name,
      title = excluded.title,
      notes = excluded.notes,
      starts_at = excluded.starts_at,
      due_at = excluded.due_at,
      completed = excluded.completed,
      completed_at = excluded.completed_at,
      priority = excluded.priority,
      raw = excluded.raw,
      last_seen_at = now()
  `
}
