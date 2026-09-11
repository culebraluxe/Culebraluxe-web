#!/usr/bin/env node
// ---------------------------------------------------------------------------
// DATA CORRECTION — client display names that made "find client" fail.
//
// Two defects were found on 2026-09-10 while chasing "Juan A. Santa Cruz":
//
//  1. DOUBLED SPACES FROM THE APPLE CONTACTS PROJECTION.
//     Apple exports absent name parts as EMPTY STRINGS, and
//     scripts/project-apple-contacts.ts joined them with concat_ws, which skips
//     NULLs but not empty strings: "Art" + " " + "" + " " + "Buyer" = "Art  Buyer".
//     469 canonical persons still carry that doubled whitespace. The projection is
//     fixed at the source (commit dcef600); this collapses the existing rows so the
//     contiguous ILIKE behind /api/portal/clients can match a normally-typed name.
//
//  2. REVIEWED NAME CORRECTIONS (explicit allow-list below).
//     A canonical Person keeps whatever name it was FIRST created with; mastery
//     links identities (phone/email) but never names the person, and the naming pass
//     (db/enrich-people.ts) SKIPS any display_name that already looks human. So a
//     person misnamed from weaker evidence can never be corrected by Contacts.
//     Person e6e52b6f was "Puerple  House" (source_evidence, inferred) while Apple
//     Contacts — the human-authored source — says "Juan A. Santa Cruz", and its
//     person_identity rows already hold his 2 emails + phone. Corrected here.
//
// DELIBERATELY NOT TOUCHED — the other 33 "genuinely different" contacts names.
//   A blanket "Contacts wins" rule would be WRONG: the sample contains
//   [Jose Sanchez Rubio] -> [Title Search Lawyer] and
//   [Michelle  M&M] -> [Culebra Sunrise Real Estate] — roles/companies, not names.
//   Those need a provenance decision, not a script.
//
// Idempotent: whitespace collapse and the allow-list both re-run safely.
// Dry-run by default; pass --apply. --env dev|prod (fail-closed), like promote.
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'
// NOTE: db/client-read-models is imported DYNAMICALLY inside main(), after APP_ENV is
// set. A static import here would resolve db/client's target BEFORE the assignment and
// silently refresh the OTHER environment's read models (observed 2026-09-10: the PROD
// run's refresh landed on DEV, leaving PROD's mv_client_directory stale).

function flag(name) {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}

const ENV = (flag('--env') ?? 'dev').toLowerCase()
const APPLY = process.argv.includes('--apply')
const SOURCE = 'apple_contacts'

/** Reviewed corrections: person id -> { name, why }. Never guess here. */
const CORRECTIONS = [
  {
    id: 'e6e52b6f-654f-4056-a8cc-0292edbaaf8b',
    name: 'Juan A. Santa Cruz',
    why: 'Apple Contacts says "Juan A. Santa Cruz"; person_identity already holds his 2 emails + phone; was misnamed "Puerple  House" from source_evidence',
  },
]

if (ENV !== 'dev' && ENV !== 'prod') {
  console.error(`--env must be dev or prod (got ${ENV})`)
  process.exit(2)
}
if (ENV === 'prod') {
  if (!process.env.DATABASE_URL_PROD) {
    console.error('No DATABASE_URL_PROD configured (fail closed)')
    process.exit(2)
  }
  if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
    console.error('PROD selected but the configured connection is the DEV URL (fail closed)')
    process.exit(2)
  }
  // db/client resolves lazily on first use — set before refreshClientReadModels().
  process.env.APP_ENV = 'production'
}

const url = ENV === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`No DATABASE_URL_${ENV.toUpperCase()} configured (fail closed)`)
  process.exit(2)
}

const pool = new Pool({ connectionString: url, ssl: true })


async function main() {
  console.log(`\nCLIENT DISPLAY-NAME CORRECTION (env=${ENV}, apply=${APPLY})`)

  // ---- 1. doubled whitespace -------------------------------------------------
  const before = await pool.query(
    `select count(*)::int n from person
      where archived_at is null and display_name ~ '  '`,
  )
  console.log(`\n[1] person rows with doubled whitespace: ${before.rows[0].n}`)
  const sample = await pool.query(
    `select display_name from person
      where archived_at is null and display_name ~ '  ' order by display_name limit 4`,
  )
  for (const r of sample.rows) console.log(`      e.g. [${r.display_name}]`)
  if (APPLY) {
    const updated = await pool.query(
      `update person
          set display_name = btrim(regexp_replace(display_name, '\\s+', ' ', 'g')),
              updated_at = now()
        where archived_at is null and display_name ~ '  '
        returning id`,
    )
    console.log(`      corrected: ${updated.rowCount}`)
  }

  // ---- 2. reviewed corrections ----------------------------------------------
  console.log(`\n[2] reviewed corrections: ${CORRECTIONS.length}`)
  for (const c of CORRECTIONS) {
    const cur = await pool.query(
      `select display_name, display_name_source from person where id = $1`,
      [c.id],
    )
    if (!cur.rowCount) {
      console.log(`      SKIP ${c.id.slice(0, 8)} (no such person)`)
      continue
    }
    const have = cur.rows[0].display_name
    const src = cur.rows[0].display_name_source
    if (have === c.name) {
      console.log(`      already correct: [${have}]`)
      continue
    }
    console.log(`      ${c.id.slice(0, 8)}: [${have}] -> [${c.name}]  (source ${src ?? 'null'} -> ${SOURCE})`)
    if (APPLY) {
      await pool.query(
        `update person set display_name = $2, display_name_source = $3, updated_at = now() where id = $1`,
        [c.id, c.name, SOURCE],
      )
    }
  }

  // ---- 3. refresh the read models the client search reads --------------------
  if (APPLY) {
    console.log('\n[3] refreshing client read models (mv_client_directory search_text)')
    const { refreshClientReadModels } = await import('../db/client-read-models')
    await refreshClientReadModels()
    console.log('      refreshed')
  }

  const after = await pool.query(
    `select count(*)::int n from person where archived_at is null and display_name ~ '  '`,
  )
  console.log(`\nRESULT: doubled-whitespace persons now ${after.rows[0].n} (was ${before.rows[0].n})`)
  console.log(APPLY ? 'APPLIED' : 'DRY RUN — re-run with --apply to write')
}

main()
  .then(() => pool.end())
  .catch(async (err) => {
    console.error(err)
    await pool.end().catch(() => {})
    process.exit(1)
  })
