// Database-side schema parity reader (Forge workspace is outside the ARCH scan
// that forbids the Neon driver in db|lib|app).
//
// Used by:
//   * scripts/check-schema-parity.ts            (`pnpm db:parity`, the release gate)
//   * workflow_app/forge/release-operations.ts  (the Forge DEV_OPS gate)
import { forgeDb, forgeDbTargetForUrl } from '../../db/forge-db'
import { compareSnapshots, type ParityReport, type SchemaSnapshot } from '../../lib/schema-parity'

export type { ParityReport } from '../../lib/schema-parity'

export async function readSchemaSnapshot(connectionString: string): Promise<SchemaSnapshot> {
  const pool = forgeDb.forTarget(forgeDbTargetForUrl(connectionString))
  try {
    const tables = (
      await pool.query(
        "select table_name from information_schema.tables where table_schema='public' and table_type='BASE TABLE' order by 1",
      )
    ).rows.map((r) => r.table_name)

    const columns = new Map<string, Map<string, string>>()
    for (const c of (
      await pool.query(
        "select table_name, column_name, data_type, is_nullable from information_schema.columns where table_schema='public' order by 1,2",
      )
    ).rows) {
      if (!columns.has(c.table_name)) columns.set(c.table_name, new Map())
      columns.get(c.table_name)!.set(c.column_name, `${c.data_type}${c.is_nullable === 'NO' ? ' NOT NULL' : ''}`)
    }

    const indexes = new Map<string, string>()
    for (const r of (
      await pool.query("select tablename, indexname, indexdef from pg_indexes where schemaname='public' order by 1,2")
    ).rows) {
      indexes.set(`${r.tablename}.${r.indexname}`, String(r.indexdef).replace(/\s+/g, ' ').trim())
    }

    const fks = new Map<string, string>()
    for (const r of (
      await pool.query(`
        select con.conname, con.conrelid::regclass::text as child, con.confrelid::regclass::text as parent
        from pg_constraint con join pg_namespace n on n.oid = con.connamespace
        where con.contype = 'f' and n.nspname = 'public' order by 1`)
    ).rows) {
      fks.set(r.conname, `${r.child} -> ${r.parent}`)
    }

    return { tables, columns, indexes, fks }
  } finally {
    await pool.end()
  }
}

export async function checkSchemaParity(devUrl: string, prodUrl: string): Promise<ParityReport> {
  const [dev, prod] = await Promise.all([readSchemaSnapshot(devUrl), readSchemaSnapshot(prodUrl)])
  return compareSnapshots(dev, prod)
}
