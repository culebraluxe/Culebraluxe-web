import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Shared interactive-transaction adapter for the Neon database.
//
// The Neon HTTP driver (`neon()`) exposes a batch-only `transaction()`; it
// cannot run interactive multi-statement transactions where one query's result
// feeds the next (the workflow engine and the canonical command receipts both
// require this). This module backs interactive transactions with the WebSocket
// `Pool` driver instead, and presents the same Neon-shaped tagged-template
// surface (including nested-fragment flattening and lazy thenables).
//
// The Pool is created lazily so importing this module never requires a
// DATABASE_URL (important for tests and module-load safety).
// ---------------------------------------------------------------------------

let poolPromise: Promise<any> | null = null

function getPool(): Promise<any> {
  if (!poolPromise) {
    poolPromise = (async () => {
      const { Pool } = await import('@neondatabase/serverless')
      const { databaseUrl } = await import('../db/client')
      return new Pool({ connectionString: databaseUrl })
    })()
  }
  return poolPromise
}

type Fragment = { strings: readonly string[]; values: any[] }

function isFragment(v: unknown): v is Fragment {
  return (
    v != null &&
    typeof v === 'object' &&
    Array.isArray((v as Fragment).strings) &&
    Array.isArray((v as Fragment).values)
  )
}

export function flatten(
  strings: readonly string[],
  values: any[],
): { text: string; params: any[] } {
  let text = ''
  const params: any[] = []
  const walk = (s: readonly string[], v: any[]) => {
    for (let i = 0; i < s.length; i++) {
      text += s[i]
      if (i < v.length) {
        const val = v[i]
        if (isFragment(val)) {
          walk(val.strings, val.values)
        } else {
          params.push(val)
          text += '$' + params.length
        }
      }
    }
  }
  walk(strings, values)
  return { text, params }
}

type Row = Record<string, any>

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
  getPool().then((pool) =>
    pool.query(text, params).then((r: any) => r.rows as Row[]),
  )

/** Neon-shaped tagged-template handle for single queries + `begin`. */
export const interactiveSql: QueryExecutor & {
  begin: (cb: (tx: QueryExecutor) => Promise<unknown>) => Promise<unknown>
} = makeQueryFn(runQuery)

interactiveSql.begin = async (cb) => {
  const pool = await getPool()
  const client = await pool.connect()
  const tx = makeQueryFn((text, params) =>
    client.query(text, params).then((r: any) => r.rows as Row[]),
  )
  try {
    await client.query('BEGIN')
    const result = await cb(tx)
    await client.query('COMMIT')
    return result
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

/** Run an application command's interactive transaction body. */
export async function withTransaction<T>(
  cb: (tx: QueryExecutor) => Promise<T>,
): Promise<T> {
  return interactiveSql.begin(cb) as Promise<T>
}
