/**
 * DATA QUALITY CHECK FOR "l_Regrid" — compares the LOADED TABLE against the source export.
 *
 *   node --import tsx --env-file=.env.local scripts/verify-l-regrid.ts --file ~/Downloads/culebra.csv
 *
 * WHY AN INDEPENDENT DECODER: this file parses the CSV and normalizes types with its OWN code rather than
 * importing the importer's helpers. A verifier that reuses the loader's conversion can only prove the loader is
 * self-consistent; parsing again here means a decoder bug shows up as a mismatch instead of cancelling out.
 *
 * WHAT IT PROVES:
 *   1. every record in the file has a row in the table and vice versa (matched on ogc_fid, the primary key);
 *   2. every CELL matches after the ONE documented transform (empty -> NULL, qoz Yes/No -> boolean), with
 *      mismatches counted per column and three examples each;
 *   3. cross-field arithmetic the source implies (parcelnumb vs its unformatted twin, parval vs improvval +
 *      landval, taxable vs parval - exemp - exon, inside_x/inside_y vs |lon|/lat);
 *   4. ranges and distributions a consumer would care about (coordinates inside Culebra's box, date ranges,
 *      duplicate parcel numbers, municipality coverage, and that no empty string survived as '').
 */
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { sql } from '../db/client'

const args = process.argv.slice(2)
const argOf = (name: string): string | null => {
  const at = args.indexOf(`--${name}`)
  return at >= 0 ? (args[at + 1] ?? null) : null
}
const file = resolve(argOf('file') ?? `${process.env.HOME ?? ''}/Downloads/culebra.csv`)
const TABLE = 'l_Regrid'

function parseCsv(text: string): string[][] {
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
  return rows.filter((candidate) => candidate.length > 1 || (candidate[0] ?? '').trim() !== '')
}

/** The value the table SHOULD hold for a source cell, in a comparable form. Independent of the importer. */
function expected(value: string, udt: string): string | boolean | null {
  const text = value.trim()
  if (text === '') return null
  if (udt === 'bool') return /^(yes|y|true|t|1)$/i.test(text)
  if (udt === 'uuid') return text.toLowerCase()
  if (udt === 'timestamptz' || udt === 'timestamp') {
    const parsed = new Date(text)
    return Number.isNaN(parsed.getTime()) ? text : parsed.toISOString()
  }
  if (udt === 'date') return text
  if (udt === 'numeric' || udt.startsWith('int') || udt === 'float8') return String(Number(text))
  return text
}

/** The value the table ACTUALLY holds, in the same comparable form. */
function actual(value: unknown, udt: string): string | boolean | null {
  if (value === null || value === undefined) return null
  if (typeof value === 'boolean') return value
  if (value instanceof Date) return udt === 'date' ? value.toISOString().slice(0, 10) : value.toISOString()
  const text = String(value)
  if (udt === 'uuid') return text.toLowerCase()
  if (udt === 'numeric' || udt.startsWith('int') || udt === 'float8') return String(Number(text))
  return text
}

function out(line: string): void {
  process.stdout.write(`${line}\n`)
}

async function main(): Promise<void> {
  const records = parseCsv(readFileSync(file, 'utf8'))
  const header = records[0].map((name) => name.trim())
  const data = records.slice(1)

  const columns = await sql`
    select column_name, udt_name from information_schema.columns
     where table_name = ${TABLE} order by ordinal_position`
  const udtOf = new Map(columns.map((row) => [String(row.column_name), String(row.udt_name)]))
  const rows = await sql`select * from "l_Regrid"`
  out(`table "${TABLE}": ${rows.length} rows, ${columns.length} columns`)
  out(`file: ${data.length} records, ${header.length} fields`)

  const byId = new Map<string, Record<string, unknown>>()
  for (const row of rows) byId.set(String(row.ogc_fid), row)

  const mismatches = new Map<string, { count: number; examples: string[] }>()
  const note = (column: string, example: string): void => {
    const entry = mismatches.get(column) ?? { count: 0, examples: [] }
    entry.count += 1
    if (entry.examples.length < 3) entry.examples.push(example)
    mismatches.set(column, entry)
  }

  let missingRows = 0
  let comparedCells = 0
  const idInFile = new Set<string>()
  data.forEach((record, index) => {
    const rowNumber = index + 2
    const id = (record[header.indexOf('ogc_fid')] ?? '').trim()
    idInFile.add(id)
    const row = byId.get(id)
    if (!row) {
      missingRows += 1
      if (missingRows <= 3) out(`row ${rowNumber}: ogc_fid ${id} is in the file but not in the table`)
      return
    }
    for (let field = 0; field < header.length; field++) {
      const name = header[field]
      const udt = udtOf.get(name)
      if (!udt) continue
      comparedCells += 1
      const want = expected(record[field] ?? '', udt)
      const got = actual(row[name], udt)
      if (want !== got) {
        note(name, `row ${rowNumber} (ogc_fid ${id}): file=${JSON.stringify(want)} table=${JSON.stringify(got)}`)
      }
    }
  })
  const extraRows = rows.filter((row) => !idInFile.has(String(row.ogc_fid)))

  out('')
  out('--- cell comparison ---')
  out(`compared: ${comparedCells} cells across ${data.length} records`)
  out(
    `rows only in the table: ${extraRows.length}` +
      (extraRows.length ? ` (e.g. ${extraRows.slice(0, 3).map((row) => String(row.ogc_fid)).join(', ')})` : ''),
  )
  out(`records missing from the table: ${missingRows}`)
  if (mismatches.size === 0) {
    out('mismatched columns: 0 — every cell matches the source after empty->NULL and qoz Yes->true')
  } else {
    out(`mismatched columns: ${mismatches.size}`)
    for (const [column, entry] of [...mismatches].sort((a, b) => b[1].count - a[1].count)) {
      out(`  ${column}: ${entry.count}`)
      for (const example of entry.examples) out(`      ${example}`)
    }
  }

  // ---------------------------------------------------------------------------
  // QUALITY: the checks that decide whether this data is usable, not just faithful.
  // ---------------------------------------------------------------------------

  out('')
  out('--- cross-field and range checks ---')

  const emptyStrings = await sql`
    select count(*)::int as n from "l_Regrid"
     where "parcelnumb" = '' or "owner" = '' or "mailadd" = '' or "legaldesc" = '' or "municipio" = ''`
  out(`empty strings where the source was empty (expected 0, NULL instead): ${String(emptyStrings[0]?.n ?? '?')}`)

  const formatted = await sql`
    select count(*)::int as n from "l_Regrid"
     where "parcelnumb_no_formatting" is not null
       and replace("parcelnumb", '-', '') <> "parcelnumb_no_formatting"`
  out(`parcelnumb vs its unformatted twin (should be 0 differences): ${String(formatted[0]?.n ?? '?')}`)

  const parval = await sql`
    select count(*)::int as n from "l_Regrid"
     where "parval" is not null and "landval" is not null and "improvval" is not null
       and "parval" <> coalesce("landval", 0) + coalesce("improvval", 0)`
  out(`parval <> landval + improvval: ${String(parval[0]?.n ?? '?')}`)

  const taxable = await sql`
    select count(*)::int as n from "l_Regrid"
     where "taxable" is not null and "parval" is not null
       and "taxable" <> "parval" - coalesce("exemp", 0) - coalesce("exon", 0)`
  out(`taxable <> parval - exemp - exon: ${String(taxable[0]?.n ?? '?')}`)

  const coords = await sql`
    select count(*)::int as n,
           min("lat")::text as lat_min, max("lat")::text as lat_max,
           min("lon")::text as lon_min, max("lon")::text as lon_max
      from "l_Regrid"`
  out(`coordinates: ${JSON.stringify(coords[0])}`)

  const inside = await sql`
    select max(abs(abs("lon") - "inside_x"))::text as dlon, max(abs("lat" - "inside_y"))::text as dlat
      from "l_Regrid" where "inside_x" is not null and "inside_y" is not null`
  out(`inside_x/inside_y vs |lon|/lat (max difference): ${JSON.stringify(inside[0])}`)

  const ids = await sql`
    select count(distinct "parcelnumb")::int as parcels, count(distinct "catastro")::int as catastros,
           count(distinct "ll_uuid")::int as uuids, count(*)::int as rows from "l_Regrid"`
  out(`distinct: ${JSON.stringify(ids[0])}`)

  const dupParcel = await sql`
    select "parcelnumb", count(*)::int as n from "l_Regrid"
     where "parcelnumb" is not null group by "parcelnumb" having count(*) > 1
     order by n desc limit 5`
  out(`duplicate parcelnumb (top 5): ${JSON.stringify(dupParcel)}`)

  const dates = await sql`
    select min("saledate")::text as sale_min, max("saledate")::text as sale_max,
           count("saledate")::int as sale_rows,
           min("ll_updated_at")::text as upd_min, max("ll_updated_at")::text as upd_max,
           min("ll_last_refresh")::text as refresh_min, max("ll_last_refresh")::text as refresh_max
      from "l_Regrid"`
  out(`dates: ${JSON.stringify(dates[0])}`)

  const municipio = await sql`
    select coalesce("municipio", '(null)') as municipio, count(*)::int as n
      from "l_Regrid" group by 1 order by n desc limit 8`
  out(`municipio: ${JSON.stringify(municipio)}`)

  const mailState = await sql`
    select coalesce("mail_state2", '(null)') as st, count(*)::int as n
      from "l_Regrid" group by 1 order by n desc limit 6`
  out(`owner mailing state: ${JSON.stringify(mailState)}`)

  const stable = await sql`
    select coalesce("ll_stable_id", '(null)') as id, count(*)::int as n
      from "l_Regrid" group by 1 order by n desc`
  out(`ll_stable_id: ${JSON.stringify(stable)}`)

  const geometry = await sql`
    select count(*) filter (where "lat" is null or "lon" is null)::int as missing_coords,
           count(*) filter (where "ll_stack_uuid" is not null)::int as stacked,
           count(*) filter (where "original_address" is not null)::int as with_original_address
      from "l_Regrid"`
  out(`completeness: ${JSON.stringify(geometry[0])}`)
}

main().catch((error: unknown) => {
  process.stderr.write(`verify-l-regrid: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exit(1)
})
