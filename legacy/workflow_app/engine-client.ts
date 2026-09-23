import { interactiveSql } from '@/lib/neon-interactive'
import type { QueryExecutor } from '@/legacy/db/query-executor'

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

/**
 * Is this failure "the relation does not exist" — i.e. the engine's tables are absent from this database?
 *
 * TWO SHAPES, AND THE SECOND ONE WAS INVISIBLE. The engine handle (`engineSql`) is a raw Neon client, so its errors carry the
 * SQLSTATE on `.code` and the original check worked. But `legacy/db`'s contained `sql` NORMALIZES failures: it throws a
 * `DbFailureError` whose message is the failure KIND ('UNKNOWN', 'SCHEMA_MISMATCH', …) and whose SQLSTATE lives inside
 * `.failure`. A caller reading only `.code` never sees 42P01 through that executor, so its "the tables are absent" branch is
 * unreachable — the read throws instead, and the screen answers 500 for a state it has a panel drawn for.
 *
 * NOTHING ELSE IS SWALLOWED. 42703 (undefined column) is schema DRIFT and deliberately not treated as absence: the kind
 * mapping folds both to SCHEMA_MISMATCH, which is why that kind is not used here and the exact code is.
 */
export function isMissingRelation(err: unknown): boolean {
  if (typeof err !== 'object' || err === null) return false
  if ((err as { code?: string }).code === '42P01') return true
  const failure = (err as { failure?: { code?: string } }).failure
  return failure?.code === '42P01'
}
