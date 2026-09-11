// ---------------------------------------------------------------------------
// REL-INTEL — enrich canonical Person display names from Apple Contacts staged
// data (CORE RULE: IDENTITY IS NOT DISPLAY NAME).
//
// Matches Apple Contacts l_person identities (normalized) to canonical Person
// identities and, when a trusted human name is available, updates the canonical
// person.display_name (and carries supported contact fields like location).
// Phone/email stay in person_identity. Unresolvable identities are marked
// 'unresolved' instead of treating the identity string as a client name.
// Idempotent and replay-safe — run as many times as you like.
//
// ENV: DEV is the default (unchanged behaviour). --env prod is EXPLICIT and
// fail-closed, mirroring scripts/promote-apple-contacts.ts, because PROD needs
// this pass too: mastering links identities but never names the person, so a
// canonical Person keeps its first-seen name (2026-09-10: a contact named
// "Juan A. Santa Cruz" stayed mastered onto a Person called "Puerple  House",
// making the client unfindable by name).
//
//   node --env-file=.env.local --import tsx scripts/enrich-apple-contacts-names.ts              # DEV
//   node --env-file=.env.local --import tsx scripts/enrich-apple-contacts-names.ts --env prod   # PROD (fail-closed)
// ---------------------------------------------------------------------------
import { sql } from '../db/client'
import { enrichDisplayNamesFromAppleContacts } from '../db/enrich-people'

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}

async function main(): Promise<void> {
  const env = (flag('--env') ?? 'dev').toLowerCase()
  if (env !== 'dev' && env !== 'prod') {
    console.error(`--env must be dev or prod (got ${env})`)
    process.exit(2)
  }
  if (env === 'prod') {
    if (!process.env.DATABASE_URL_PROD) {
      console.error('No DATABASE_URL_PROD configured (fail closed)')
      process.exit(2)
    }
    if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
      console.error('PROD selected but the configured connection is the DEV URL (fail closed)')
      process.exit(2)
    }
    // db/client resolves its target LAZILY on first use, so this must be set
    // before the first query below — same pattern as promote-apple-contacts.ts.
    process.env.APP_ENV = 'production'
  }
  console.log(`\nENRICH Apple Contacts names -> canonical Person (env=${env})`)

  const before = (await sql`
    select display_name_source, count(*)::int as n
    from person where archived_at is null
    group by display_name_source order by display_name_source
  `) as { display_name_source: string | null; n: number }[]

  const result = await enrichDisplayNamesFromAppleContacts()

  const after = (await sql`
    select display_name_source, count(*)::int as n
    from person where archived_at is null
    group by display_name_source order by display_name_source
  `) as { display_name_source: string | null; n: number }[]

  const sample = (await sql`
    select display_name, display_name_source, location
    from person
    where archived_at is null
      and display_name ~ '^[+0-9()\\s.-]+$'
    order by display_name
    limit 5
  `) as { display_name: string; display_name_source: string | null; location: string | null }[]

  console.log('  enriched with a contact human name:', result.enriched)
  console.log('  ambiguous (multiple names, left unresolved):', result.ambiguous)
  console.log('  marked unresolved (safe fallback):', result.unresolved)
  console.log('  already-human marked source_evidence:', result.resolvedHuman)
  console.log('\n  display_name_source before:')
  for (const row of before) console.log(`    ${row.display_name_source ?? '(null)'}: ${row.n}`)
  console.log('  display_name_source after:')
  for (const row of after) console.log(`    ${row.display_name_source ?? '(null)'}: ${row.n}`)
  console.log('\n  remaining phone-number display names (should be 0 if all resolvable):')
  console.log('   ', sample.length)
  for (const row of sample) console.log(`    ${row.display_name} (${row.display_name_source ?? 'null'})`)
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
