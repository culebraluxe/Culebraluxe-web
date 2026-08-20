import { neon } from '@neondatabase/serverless'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Handle to the workflow engine's own Postgres database.
//
// The engine persistence layer is intentionally separate from the CulebraLuxe
// schema. If the engine database is not configured, the Portal renders a
// setup/empty state rather than failing.
// ---------------------------------------------------------------------------

const engineUrl = process.env.WORKFLOW_ENGINE_DATABASE_URL

let cached: ReturnType<typeof neon> | null = null

export function engineConfigured(): boolean {
  return Boolean(engineUrl)
}

export function engineSql(): QueryExecutor {
  if (!engineUrl) {
    throw new Error(
      'Workflow engine database is not configured (WORKFLOW_ENGINE_DATABASE_URL).',
    )
  }
  if (!cached) cached = neon(engineUrl)
  return cached as unknown as QueryExecutor
}
