import { Pool } from '@neondatabase/serverless'
import { databaseUrl } from '../db/client'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Workflow engine persistence handle — SAME Neon database as CulebraLuxe.
//
// CTO topology: ONE database hosts both the application tables and the engine
// tables. The code boundary is unchanged (the engine still knows nothing about
// deal/offer/property/person).
//
// Driver seam: the engine uses a Neon-style SQL handle:
//   - `sql\`...\``            tagged template (with nested fragments)
//   - `sql.begin(async tx)`  interactive transaction, `tx\`...\`` inside
//
// The Neon HTTP driver (`neon()`) is non-interactive (its `transaction()` is a
// batch API), so the engine's interactive `begin` is backed by the WebSocket
// `Pool` driver instead. This adapter presents the Neon tagged-template shape
// (including nested fragment flattening) over `Pool.query(text, params)`.
// ---------------------------------------------------------------------------

const pool = new Pool({ connectionString: databaseUrl })

type Fragment = { strings: readonly string[]; values: any[] }

function isFragment(v: unknown): v is Fragment {
  return (
    v != null &&
    typeof v === 'object' &&
    Array.isArray((v as Fragment).strings) &&
    Array.isArray((v as Fragment).values)
  )
}

/** Flatten a tagged template (with nested fragments) into (text, params). */
function flatten(
  strings: TemplateStringsArray,
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

/** Build a Neon-shaped tagged-template query function over a pg-style runner. */
function makeQueryFn(run: (text: string, params: any[]) => Promise<Row[]>) {
  const fn: any = (strings: TemplateStringsArray, ...values: any[]) => {
    const { text, params } = flatten(strings, values)
    let memo: Promise<Row[]> | null = null
    // Lazy thenable: nested `sql\`...\`` fragments carry `.strings`/`.values`
    // for flattening but do NOT execute until awaited. This matches the Neon
    // tagged-template contract the engine relies on.
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

const engineHandle: QueryExecutor & { begin: (cb: (tx: QueryExecutor) => Promise<unknown>) => Promise<unknown> } =
  makeQueryFn((text, params) => pool.query(text, params).then((r) => r.rows as Row[]))

engineHandle.begin = async (cb) => {
  const client = await pool.connect()
  const tx = makeQueryFn((text, params) =>
    client.query(text, params).then((r) => r.rows as Row[]),
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

export function engineConfigured(): boolean {
  // Shared database: the engine is always configured when the app is.
  return true
}

export function engineSql(): QueryExecutor {
  return engineHandle as unknown as QueryExecutor
}

export function isMissingRelation(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === '42P01'
  )
}
