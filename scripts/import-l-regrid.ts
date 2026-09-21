/**
 * LOAD THE REGRID EXPORT INTO "l_Regrid" (landing table, migration 199).
 *
 *   node --import tsx --env-file=.env.local scripts/import-l-regrid.ts \
 *     --file ~/Downloads/culebra.csv --target dev [--dry-run]
 *
 * WHAT IT DOES, and the choices that are deliberate:
 *
 *   * RFC 4180 CSV: quoted fields, doubled quotes, commas inside quotes. The header row is the COLUMN LIST and
 *     is never imported as data (it would collide with the primary key and the types).
 *   * THE TYPE RULES LIVE IN ONE PLACE — the table itself. The importer reads each column's type from
 *     information_schema and validates against THAT, so the schema review that produced the DDL is not copied
 *     into a second file that can drift from it.
 *   * EMPTY IS NULL. The export writes `""` for missing; it lands as NULL so "absent" has one representation.
 *   * VALIDATION RUNS BEFORE ANY WRITE, over every record, and reports the row number, the field, the value and
 *     what was expected. A file that fails validation never touches the table.
 *   * ONE TRANSACTION, TRUNCATE FIRST. A re-run replaces the previous load instead of duplicating it, and a
 *     failure anywhere rolls the whole load back — a landing table is never left half-written.
 *   * THE LOAD PROVES ITS OWN COUNT inside the transaction: if the rows it inserted are not the rows it parsed,
 *     it throws and nothing is committed.
 */
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { withForgeTransaction, type ForgeDbTx } from '@/legacy/db/forge-db'

type Target = 'dev' | 'prod'

const TABLE = 'l_Regrid'
const BATCH = 100

const args = process.argv.slice(2)
const argOf = (name: string): string | null => {
  const at = args.indexOf(`--${name}`)
  return at >= 0 ? (args[at + 1] ?? null) : null
}
const file = resolve(argOf('file') ?? `${process.env.HOME ?? ''}/Downloads/culebra.csv`)
const target = (argOf('target') ?? (process.env.APP_ENV === 'production' ? 'prod' : 'dev')) as Target
const dryRun = args.includes('--dry-run')

/** RFC 4180: quoted fields, doubled quotes inside them, commas and newlines inside quotes. */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = []
  let row: string[] = []
  let field = ''
  let inQuotes = false
  for (let i = 0; i < text.length; i++) {
    const ch = text[i]
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"'
          i++
        } else inQuotes = false
      } else field += ch
      continue
    }
    if (ch === '"') {
      inQuotes = true
      continue
    }
    if (ch === ',') {
      row.push(field)
      field = ''
      continue
    }
    if (ch === '\n') {
      row.push(field)
      rows.push(row)
      row = []
      field = ''
      continue
    }
    if (ch === '\r') continue
    field += ch
  }
  if (field.length > 0 || row.length > 0) {
    row.push(field)
    rows.push(row)
  }
  // A trailing blank line is not a record.
  return rows.filter((candidate) => candidate.length > 1 || (candidate[0] ?? '').trim() !== '')
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/
const NUMERIC = /^-?\d+(\.\d+)?$/
const STAMP = /^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(:\d{2})?(\.\d+)?(\s*[+-]\d{2}(:?\d{2})?)?$/

/**
 * Convert one source value for one column type, or return the reason it cannot be stored. `udt_name` is what
 * information_schema reports, so this follows the TABLE rather than a second copy of the schema review.
 */
export function convert(
  value: string,
  udt: string,
): { ok: true; value: string | boolean | null } | { ok: false; reason: string } {
  const text = value.trim()
  if (text === '') return { ok: true, value: null }
  if (udt === 'text' || udt === 'varchar' || udt === 'bpchar') return { ok: true, value: text }
  if (udt === 'bool') {
    if (/^(yes|y|true|t|1)$/i.test(text)) return { ok: true, value: true }
    if (/^(no|n|false|f|0)$/i.test(text)) return { ok: true, value: false }
    return { ok: false, reason: `not a boolean: ${JSON.stringify(text)}` }
  }
  if (udt === 'uuid') return UUID.test(text) ? { ok: true, value: text } : { ok: false, reason: 'not a uuid' }
  if (udt === 'date') return ISO_DATE.test(text) ? { ok: true, value: text } : { ok: false, reason: 'not an ISO date (YYYY-MM-DD)' }
  if (udt === 'timestamptz' || udt === 'timestamp') {
    return STAMP.test(text) ? { ok: true, value: text } : { ok: false, reason: 'not a timestamp' }
  }
  if (udt === 'numeric' || udt === 'int8' || udt === 'int4' || udt === 'int2' || udt === 'float8') {
    if (!NUMERIC.test(text)) return { ok: false, reason: 'not a number' }
    if (udt.startsWith('int') && !/^-?\d+$/.test(text)) return { ok: false, reason: 'not a whole number' }
    return { ok: true, value: text }
  }
  return { ok: false, reason: `unsupported column type ${udt}` }
}

type Column = { name: string; udt: string }

async function tableColumns(tx: ForgeDbTx): Promise<Column[]> {
  const rows = await tx.query(
    `select column_name, udt_name from information_schema.columns
      where table_name = $1 order by ordinal_position`,
    [TABLE],
  )
  return rows.rows.map((row) => ({ name: String(row.column_name), udt: String(row.udt_name) }))
}

function out(line: string): void {
  process.stdout.write(`${line}\n`)
}

async function main(): Promise<void> {
  const raw = readFileSync(file, 'utf8')
  const rows = parseCsv(raw)
  if (rows.length < 2) throw new Error(`${file} has no data rows`)
  const header = rows[0].map((name) => name.trim())
  const records = rows.slice(1)
  out(`file: ${file}`)
  out(`parsed: ${records.length} records, ${header.length} fields (header row excluded from the load)`)

  const parsed = await withForgeTransaction(async (tx) => {
    const columns = await tableColumns(tx)
    if (columns.length === 0) throw new Error(`table "${TABLE}" does not exist — apply migration 199 first`)
    const byName = new Map(columns.map((column) => [column.name, column]))
    const missing = header.filter((name) => !byName.has(name))
    if (missing.length > 0) {
      throw new Error(`the file has fields the table does not: ${missing.join(', ')}`)
    }
    const absentFromFile = columns.filter((column) => !header.includes(column.name)).map((column) => column.name)
    out(`columns: ${columns.length} in the table, ${header.length} in the file`)
    if (absentFromFile.length > 0) out(`warning: loaded as NULL, absent from this export: ${absentFromFile.join(', ')}`)

    // VALIDATE EVERY RECORD BEFORE ANY WRITE.
    const failures: string[] = []
    const converted: Array<Array<string | boolean | null>> = []
    records.forEach((record, index) => {
      if (record.length !== header.length) {
        if (failures.length < 10) failures.push(`row ${index + 2}: ${record.length} fields, expected ${header.length}`)
        converted.push([])
        return
      }
      const tuple = header.map((name, field) => {
        const column = byName.get(name) as Column
        const result = convert(record[field] ?? '', column.udt)
        if (!result.ok) {
          if (failures.length < 10) {
            failures.push(`row ${index + 2} field "${name}": ${result.reason} (udt=${column.udt})`)
          }
          return null
        }
        return result.value
      })
      converted.push(tuple)
    })
    if (failures.length > 0) {
      throw new Error(`validation failed:\n  ${failures.join('\n  ')}`)
    }
    out(`validated: ${converted.length} records against the table's own column types`)

    if (dryRun) {
      out('dry run: nothing written')
      return 0
    }

    // TRUNCATE + LOAD, ONE TRANSACTION: a re-run replaces the load, and a failure leaves the table as it was.
    const columnList = header.map((name) => `"${name}"`).join(', ')
    await tx.query(`truncate table "${TABLE}"`)
    let inserted = 0
    for (let start = 0; start < converted.length; start += BATCH) {
      const batch = converted.slice(start, start + BATCH)
      const params: Array<string | boolean | null> = []
      const tuples = batch.map((tuple) => {
        params.push(...tuple)
        const base = params.length - header.length
        return `(${header.map((_, index) => `$${base + index + 1}`).join(', ')})`
      })
      const result = await tx.query(
        `insert into "${TABLE}" (${columnList}) values ${tuples.join(', ')}`,
        params,
      )
      inserted += result.rowCount ?? 0
    }

    // THE LOAD PROVES ITS OWN COUNT, inside the transaction: a mismatch throws and nothing is committed.
    const counted = await tx.query(`select count(*)::int as n, count(distinct "ogc_fid")::int as ids from "${TABLE}"`)
    const total = Number(counted.rows[0]?.n ?? -1)
    const distinctIds = Number(counted.rows[0]?.ids ?? -1)
    if (total !== converted.length || inserted !== converted.length) {
      throw new Error(`loaded ${inserted} rows, table holds ${total}, file has ${converted.length} records`)
    }
    if (distinctIds !== total) throw new Error(`${total} rows but only ${distinctIds} distinct ogc_fid values`)
    out(`loaded: ${inserted} rows into "${TABLE}" (${distinctIds} distinct ogc_fid)`)
    return inserted
  }, target)

  out(`target: ${target} — done (${parsed} records)`)
}

main().catch((error: unknown) => {
  process.stderr.write(`import-l-regrid: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exit(1)
})
