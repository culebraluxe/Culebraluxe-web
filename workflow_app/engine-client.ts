import { sql } from '../db/client'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Workflow engine persistence handle — SAME Neon database as CulebraLuxe.
//
// CTO topology: ONE database hosts both the application tables and the engine
// tables. The code boundary is unchanged (the engine still knows nothing about
// deal/offer/property/person). The engine expects `sql.begin(cb)`; Neon exposes
// `sql.transaction(cb)`, so this module adapts that single seam.
// ---------------------------------------------------------------------------

const engineHandle = (() => {
  const q = sql as any
  if (typeof q.begin !== 'function') {
    q.begin = (cb: (tx: QueryExecutor) => Promise<unknown>) =>
      q.transaction(async (tx: any) => cb(tx as QueryExecutor))
  }
  return q
})()

export function engineConfigured(): boolean {
  // Shared database: the engine is always configured when the app is.
  return true
}

export function engineSql(): QueryExecutor {
  return engineHandle as QueryExecutor
}

export function isMissingRelation(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === '42P01'
  )
}
