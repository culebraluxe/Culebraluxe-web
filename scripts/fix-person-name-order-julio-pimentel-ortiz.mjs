#!/usr/bin/env node
// ---------------------------------------------------------------------------
// DATA CORRECTION — Listing Agreement seller name order.
//
// The listing agreement for Julio Pimentel Ortiz was prepared with the two last
// names transposed ("Julio Ortiz Pimentel") because Apple Contacts had them that
// way. The contact source is fixed; this corrects OUR system (PROD, and DEV which
// mirrors it).
//
// Scope of the correction (steps 1 and 2 of the fire drill):
//   1. person.display_name                     : "Julio Ortiz Pimentel" -> "Julio Pimentel Ortiz"
//   2. document_form_instance (the LAST form)  : field_values + ALL sections free text
//
// DELIBERATELY NOT TOUCHED:
//   * transaction_document "Listing Agreement v30" and its rendered PDF media.
//     The issued version's sha256 checksum is the hash of the RENDERED PDF
//     (db/issued-document.ts), so editing its content in place would break that
//     binding and require inventing a checksum. The correct fix is to re-issue a
//     revised immutable version from the corrected form (issueFormAction), which
//     renders, hashes, appends a new media row and supersedes v30 (the house
//     pattern: 41 superseded rows).
//
// Idempotent: replaces only the transposed string; safe to re-run.
// Dry-run by default; pass --apply.
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'

const APPLY = process.argv.includes('--apply')
const WRONG = 'Julio Ortiz Pimentel'
const RIGHT = 'Julio Pimentel Ortiz'
const PERSON = 'd29c4327-e9f8-407f-9964-8e82e12b9ceb'
const FORM = 'd3206505-5ada-4be4-896b-9b382bef4e4a'

/** Replace WRONG -> RIGHT in every string of any nested JSON value. */
function deepReplace(value, counts) {
  if (typeof value === 'string') {
    if (value.includes(WRONG)) {
      counts.n += 1
      return value.split(WRONG).join(RIGHT)
    }
    return value
  }
  if (Array.isArray(value)) return value.map((v) => deepReplace(v, counts))
  if (value && typeof value === 'object') {
    const out = {}
    for (const [k, v] of Object.entries(value)) out[k] = deepReplace(v, counts)
    return out
  }
  return value
}

for (const [label, url] of [
  ['PROD', process.env.DATABASE_URL_PROD],
  ['DEV', process.env.DATABASE_URL_DEV],
]) {
  const pool = new Pool({ connectionString: url })
  const client = await pool.connect()
  try {
    const p = await client.query(`select id, display_name from person where id = $1`, [PERSON])
    const f = await client.query(
      `select id, field_values, sections from document_form_instance where id = $1`,
      [FORM],
    )
    if (p.rowCount === 0 && f.rowCount === 0) {
      console.log(`${label}: neither the person nor the form exists here — skipped`)
      continue
    }

    console.log(`\n===== ${label} =====`)
    console.log(`person.display_name  BEFORE: ${JSON.stringify(p.rows[0]?.display_name ?? '(absent)')}`)

    const counts = { n: 0 }
    const fieldValues = deepReplace(f.rows[0]?.field_values ?? {}, counts)
    const sections = deepReplace(f.rows[0]?.sections ?? {}, counts)
    console.log(`form ${FORM}: ${counts.n} string occurrence(s) of "${WRONG}" will be corrected`)
    console.log(`  field_values.sellerName BEFORE: ${JSON.stringify(f.rows[0]?.field_values?.sellerName ?? '')}`)

    if (!APPLY) continue

    await client.query('begin')
    if (p.rowCount > 0) {
      await client.query(
        `update person set display_name = $2, updated_at = now()
          where id = $1 and display_name = $3`,
        [PERSON, RIGHT, WRONG],
      )
    }
    if (f.rowCount > 0) {
      await client.query(
        `update document_form_instance
            set field_values = $2::jsonb, sections = $3::jsonb, updated_at = now()
          where id = $1`,
        [FORM, JSON.stringify(fieldValues), JSON.stringify(sections)],
      )
    }
    await client.query('commit')

    // verify
    const after = await client.query(`select display_name from person where id = $1`, [PERSON])
    const afterForm = await client.query(
      `select (field_values::text ilike '%Ortiz Pimentel%') has_field,
              (sections::text ilike '%Ortiz Pimentel%') has_section,
              field_values->>'sellerName' seller
         from document_form_instance where id = $1`,
      [FORM],
    )
    console.log(`person.display_name  AFTER : ${JSON.stringify(after.rows[0]?.display_name ?? '(absent)')}`)
    console.log(
      `form AFTER: sellerName=${JSON.stringify(afterForm.rows[0]?.seller ?? '')} | residual "Ortiz Pimentel" in field_values=${afterForm.rows[0]?.has_field} sections=${afterForm.rows[0]?.has_section}`,
    )
  } finally {
    client.release()
    await pool.end()
  }
}
console.log(APPLY ? '\nAPPLIED' : '\nDRY RUN — re-run with --apply')
