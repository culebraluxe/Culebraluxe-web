import type { QueryExecutor, QueryRow } from './query-executor'

// The app_error writer must NOT import the db/client facade (which re-exports the
// gateway), or it forms a cycle: gateway -> app-error (logFailure capture) and
// app-error -> client/gateway (executor). Instead the executor is INJECTED: the
// db/client facade registers its sql executor here once. App-error depends only
// on the QueryExecutor type — no runtime import back into the gateway.
let defaultExecutor: QueryExecutor | null = null

/** Register the executor app_error should use for writes (called by db/client). */
export function setErrorExecutor(execute: QueryExecutor | null): void {
  defaultExecutor = execute
}

/** Resolve the executor for a call: the injected one, else the registered default. */
function resolveExecutor(execute?: QueryExecutor): QueryExecutor {
  if (execute) return execute
  if (defaultExecutor) return defaultExecutor
  throw new Error(
    'app_error executor is not configured; register it via setErrorExecutor(db.sql) or pass execute explicitly.',
  )
}

export type ErrorLevel = 'info' | 'warn' | 'error' | 'fatal'

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
  level: string
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
  level?: ErrorLevel
  meta?: Record<string, unknown> | null
}

/** The durable Log4j-style capture. NEVER logs secrets/raw SQL/bind values. */
export async function recordError(
  input: RecordErrorInput,
  execute?: QueryExecutor,
): Promise<AppErrorRow> {
  const q = resolveExecutor(execute)
  const rows = await q`
    insert into app_error (kind, operation, incident_id, code, message, retryable, stack, story_id, route, level, meta)
    values (
      ${input.kind}, ${input.operation ?? null}, ${input.incidentId ?? null}, ${input.code ?? null},
      ${input.message ?? null}, ${input.retryable ?? null}, ${input.stack ?? null},
      ${input.storyId ?? null}, ${input.route ?? null}, ${input.level ?? 'error'},
      ${input.meta ? JSON.stringify(input.meta) : null}::jsonb
    )
    returning id, kind, operation, incident_id, code, message, retryable, stack, story_id, route, level, created_at
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
  level?: ErrorLevel,
  execute?: QueryExecutor,
): Promise<AppErrorRow[]> {
  const q = resolveExecutor(execute)
  const rows = level
    ? await q`
        select id, kind, operation, incident_id, code, message, retryable, stack, story_id, route, level, created_at
        from app_error
        where level = ${level}
        order by created_at desc
        limit ${Math.max(1, Math.min(limit, 200))}
      `
    : await q`
        select id, kind, operation, incident_id, code, message, retryable, stack, story_id, route, level, created_at
        from app_error
        order by created_at desc
        limit ${Math.max(1, Math.min(limit, 200))}
      `
  return rows as AppErrorRow[]
}
