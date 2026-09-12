import type { QueryExecutor } from '../db/query-executor'
import { forgeDb, forgeDbPool } from '../db/forge-db'
import { flattenSqlTemplate, toPgQuery } from '../db/sql-template'

// ---------------------------------------------------------------------------
// Interactive-transaction adapter — now a THIN LAYER OVER ForgeDB.
//
// This module used to justify its own WebSocket pool with "the Neon HTTP driver
// exposes a batch-only transaction()". That reason died with the driver: ForgeDB
// (`db/forge-db.ts`) owns the application's single `pg.Pool`, and interactive
// transactions run on one pooled client with real BEGIN/COMMIT/ROLLBACK.
//
// It also carried a COPY of the environment-resolution rule with a "Keep in sync
// with db/database-gateway.getDatabaseUrl / resolveDbTarget" comment — the exact
// shape of bug this change exists to remove. Two copies of "which database" can
// disagree; now there is one, in lib/execution-target.ts.
//
// The exported surface is unchanged (`interactiveSql`, `withTransaction`,
// `flatten`, `makeQueryFn`) because callers across `db/` and `workflow_app/` use
// it as a tagged template.
// ---------------------------------------------------------------------------

/**
 * Flatten a template (including nested fragments) into text + positional params.
 * Kept as this module's export for compatibility; the implementation is shared
 * with ForgeDB so a fragment cannot mean two different things.
 */
export function flatten(
  strings: readonly string[],
  values: any[],
): { text: string; params: any[] } {
  // Opts into the structural fragment shape this module has always accepted.
  const flat = flattenSqlTemplate(strings, values, { acceptStructuralFragments: true })
  const query = toPgQuery(flat.strings, flat.values)
  return { text: query.text, params: query.values }
}

type Row = Record<string, any>

/** Neon-shaped lazy tagged handle: matches callers that await the result. */
export function makeQueryFn(run: (text: string, params: any[]) => Promise<Row[]>) {
  const fn: any = (strings: TemplateStringsArray, ...values: any[]) => {
    const { text, params } = flatten(strings, values)
    let memo: Promise<Row[]> | null = null
    const thenable = {
      then(resolve: (rows: Row[]) => void, reject: (err: unknown) => void) {
        if (!memo) memo = run(text, params)
        return memo.then(resolve, reject)
      },
    }
    return Object.assign(thenable, { strings, values })
  }
  return fn
}

const runQuery = (text: string, params: any[]) =>
  forgeDbPool()
    .query(text, params)
    .then((r) => r.rows as Row[])

/** Tagged-template handle for single queries + `begin`, on the shared pool. */
export const interactiveSql: QueryExecutor & {
  begin: (cb: (tx: QueryExecutor) => Promise<unknown>) => Promise<unknown>
} = makeQueryFn(runQuery)

interactiveSql.begin = (cb) => forgeDb.transaction(cb)

/** Run an application command's interactive transaction body. */
export async function withTransaction<T>(
  cb: (tx: QueryExecutor) => Promise<T>,
): Promise<T> {
  return forgeDb.transaction(cb)
}
