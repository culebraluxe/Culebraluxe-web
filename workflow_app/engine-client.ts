import { interactiveSql } from '../lib/neon-interactive'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Workflow engine persistence handle — SAME Neon database as CulebraLuxe.
//
// CTO topology: ONE database hosts both the application tables and the engine
// tables. The code boundary is unchanged (the engine still knows nothing about
// deal/offer/property/person). The engine expects `sql.begin(cb)` plus a
// Neon-style tagged-template `sql\`...\``; both are provided by the shared
// interactive-transaction adapter in lib/neon-interactive.ts.
// ---------------------------------------------------------------------------

export function engineConfigured(): boolean {
  // Shared database: the engine is always configured when the app is.
  return true
}

export function engineSql(): QueryExecutor {
  return interactiveSql as unknown as QueryExecutor
}

export function isMissingRelation(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === '42P01'
  )
}
