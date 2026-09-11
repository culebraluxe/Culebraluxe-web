#!/usr/bin/env node
// ---------------------------------------------------------------------------
// PROMOTE l_person -> person  (R4: if Landing has it, Person gets it)
//
// THE BUG THIS FIXES: the Apple load has always stopped at the landing table.
// l_person has carried name, nickname, org, department, job title, note, address and
// phone/email identities for weeks — but the only thing that ever crossed into
// `person` was the NAME. So Person stayed empty and screens showed nothing.
//
// RULES (docs/agent/PERSON-PROPERTY-DESIGN.md):
//   R2 newer wins  — the landing row is the latest source state, so it wins.
//   R3 identity    — phone/email, matched on normalized values (both sides).
//   R4 everything  — name, phone, email, legal address, note all cross.
//
// Idempotent. Dry-run by default; --apply writes.
//   node --env-file=.env.local --import tsx scripts/promote-l-person-facts.ts --env prod --apply
// ---------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'

const argv = process.argv.slice(2)
const flag = (n: string) => {
  const i = argv.indexOf(n)
  return i >= 0 && i + 1 < argv.length ? argv[i + 1] : undefined
}
const ENV = (flag('--env') ?? 'dev').toLowerCase()
const APPLY = argv.includes('--apply')

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
    console.error('PROD selected but connection is the DEV URL (fail closed)')
    process.exit(2)
  }
}
const url = ENV === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`No DATABASE_URL_${ENV.toUpperCase()} configured (fail closed)`)
  process.exit(2)
}
const pool = new Pool({ connectionString: url, ssl: true })

/** The landing row for each person — newest matched revision wins (R2). */
const MATCH_SQL = `
  select distinct on (p.id)
         p.id            as person_id,
         lp.id           as l_person_id,
         lp.display_name as landing_name,
         lp.note         as landing_note,
         (select nullif(trim(concat_ws(', ',
              nullif(trim(a.street), ''),
              nullif(trim(a.city), ''),
              nullif(trim(concat_ws(' ', nullif(trim(a.state), ''), nullif(trim(a.postal_code), ''))), '')
            )), '')
            from l_person_address a
           where a.l_person_id = lp.id and a.source_label = '_$!<Home>!$_'
           limit 1)      as legal_address,
         lp.updated_at   as landing_updated_at
    from person p
    join person_identity pi on pi.person_id = p.id
    join l_person_identity li
      on li.normalized_value = pi.identity_value
      or li.identity_value   = pi.identity_value
    join l_person lp on lp.id = li.l_person_id
   where p.archived_at is null
   order by p.id, lp.updated_at desc nulls last
`

async function main() {
  console.log(`\nPROMOTE l_person -> person (env=${ENV}, apply=${APPLY})`)

  const matched = await pool.query(MATCH_SQL)
  const withAddr = matched.rows.filter((r) => r.legal_address).length
  const withNote = matched.rows.filter((r) => r.landing_note && String(r.landing_note).trim()).length
  console.log(`  persons matched to a landing row : ${matched.rowCount}`)
  console.log(`     ...carrying a legal address   : ${withAddr}`)
  console.log(`     ...carrying a note            : ${withNote}`)

  const changes = await pool.query(`
    with m as (${MATCH_SQL})
    select
      count(*) filter (where coalesce(p.display_name,'') <> coalesce(m.landing_name,''))::int as name_changes,
      count(*) filter (where m.legal_address is not null and coalesce(p.location,'') <> m.legal_address)::int as location_fills,
      count(*) filter (where coalesce(nullif(trim(m.landing_note),''),'') <> ''
                        and coalesce(p.notes,'') <> m.landing_note)::int as note_fills
    from m join person p on p.id = m.person_id
  `)
  console.log('  would change: ' + JSON.stringify(changes.rows[0]))

  if (APPLY) {
    // R2/R4 — the landing row is the latest source state, so it wins.
    const upd = await pool.query(`
      with m as (${MATCH_SQL})
      update person p set
        display_name = coalesce(nullif(trim(m.landing_name), ''), p.display_name),
        display_name_source = case when nullif(trim(m.landing_name), '') is not null
                                   then 'apple_contacts' else p.display_name_source end,
        location = coalesce(m.legal_address, p.location),
        notes = coalesce(nullif(trim(m.landing_note), ''), p.notes),
        updated_at = now()
      from m
      where p.id = m.person_id
      returning p.id
    `)
    console.log(`  PERSON updated: ${upd.rowCount}`)

    const ids = await pool.query(`
      with m as (${MATCH_SQL})
      insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
      select distinct m.person_id, li.identity_type, li.identity_value, 'apple_contacts', false
      from m
      join l_person_identity li on li.l_person_id = m.l_person_id
      where li.identity_type in ('email', 'phone')
        and not exists (
          select 1 from person_identity x
           where x.person_id = m.person_id
             and x.identity_type = li.identity_type
             and x.identity_value = li.identity_value)
      returning person_id
    `)
    console.log(`  person_identity added: ${ids.rowCount}`)
  }

  console.log(APPLY ? '\nAPPLIED' : '\nDRY RUN — re-run with --apply to write')

  if (APPLY) {
    // The view folds person.location into search_text, so it must be rebuilt after
    // facts land. Dynamic import: db/client resolves its target lazily, so APP_ENV
    // must already be set (a static import would refresh the OTHER environment).
    if (ENV === 'prod') process.env.APP_ENV = 'production'
    const { refreshClientReadModels } = await import('../db/client-read-models')
    await refreshClientReadModels()
    console.log('  read models refreshed')
  }
}

main()
  .then(() => pool.end())
  .catch(async (err) => {
    console.error(err)
    await pool.end().catch(() => {})
    process.exit(1)
  })
