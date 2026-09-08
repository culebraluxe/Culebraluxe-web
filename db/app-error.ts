import type { QueryExecutor, QueryRow } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type AppErrorRow = QueryRow & {
  id: string
  kind: string
  operation: string | null
  incident_id: string | null
  code: string | null
  message: string | null
  retryable: boolean | null
  stack: string | null
  story_id: string | null
  route: string | null
  created_at: unknown
}

export type RecordErrorInput = {
  kind: string
  operation?: string | null
  incidentId?: string | null
  code?: string | null
  message?: string | null
  retryable?: boolean | null
  stack?: string | null
  storyId?: string | null
  route?: string | null
  meta?: Record<string, unknown> | null
}

/** The durable Log4j-style capture. NEVER logs secrets/raw SQL/bind values. */
export async function recordError(
  input: RecordErrorInput,
  execute?: QueryExecutor,
): Promise<AppErrorRow> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into app_error (kind, operation, incident_id, code, message, retryable, stack, story_id, route, meta)
    values (
      ${input.kind}, ${input.operation ?? null}, ${input.incidentId ?? null}, ${input.code ?? null},
      ${input.message ?? null}, ${input.retryable ?? null}, ${input.stack ?? null},
      ${input.storyId ?? null}, ${input.route ?? null},
      ${input.meta ? JSON.stringify(input.meta) : null}::jsonb
    )
    returning id, kind, operation, incident_id, code, message, retryable, stack, story_id, route, created_at
  `
  return rows[0] as AppErrorRow
}

/** Best-effort capture that can never throw into the operation being recorded. */
export function captureError(input: RecordErrorInput): void {
  recordError(input).catch(() => {
    /* capture must never break the operation it observes */
  })
}

export async function listRecentErrors(
  limit = 25,
  execute?: QueryExecutor,
): Promise<AppErrorRow[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, kind, operation, incident_id, code, message, retryable, stack, story_id, route, created_at
    from app_error
    order by created_at desc
    limit ${Math.max(1, Math.min(limit, 200))}
  `
  return rows as AppErrorRow[]
}
