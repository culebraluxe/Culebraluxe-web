import { db } from './database-gateway'
import type { Result } from './database-gateway'

// ---------------------------------------------------------------------------
// DB-HARDEN-01 — lightweight schema capability preflight.
//
// Reusable assertion tool for deploy verification / startup diagnostic /
// migration harness / targeted production health check. It detects schema
// drift (e.g. a required column not yet promoted to a target database) BEFORE
// users hit it. This is SEPARATE from runtime graceful degradation: the runtime
// gateway still contains failures if drift somehow occurs.
//
// It queries information_schema once per table set — never on every request.
// ---------------------------------------------------------------------------

export type SchemaCapability = {
  /** default 'public' */
  schema?: string
  table: string
  column: string
}

export type SchemaPreflightResult = {
  /** Required capabilities that are MISSING on the target database. */
  missing: string[]
}

type ColumnRow = {
  table_name: string
  column_name: string
}

/** Assert the target database exposes the required schema capabilities.
 *  Returns a typed Result: ok with a `missing` list, or a gateway failure if
 *  the preflight query itself could not run (DB unavailable). */
export async function assertSchemaCapabilities(
  capabilities: SchemaCapability[],
): Promise<Result<SchemaPreflightResult>> {
  const tableNames = [...new Set(capabilities.map((c) => c.table))]
  if (tableNames.length === 0) return { ok: true, data: { missing: [] } }

  const r = await db.query<ColumnRow>`
    select table_name, column_name
    from information_schema.columns
    where table_schema = 'public'
      and table_name = any(${tableNames})
  `
  if (!r.ok) return r

  const present = new Set(
    r.data.map((row) => `${row.table_name}.${row.column_name}`),
  )
  const missing = capabilities
    .filter((c) => !present.has(`${c.table}.${c.column}`))
    .map((c) => `${c.table}.${c.column}`)

  return { ok: true, data: { missing } }
}
