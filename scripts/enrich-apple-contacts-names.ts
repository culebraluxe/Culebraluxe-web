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
// DEV only. Never touches production. Does NOT delete/reload source data.
//
//   node --env-file=.env.local --import tsx scripts/enrich-apple-contacts-names.ts
// ---------------------------------------------------------------------------
import { sql } from '../db/client'
import { enrichDisplayNamesFromAppleContacts } from '../db/enrich-people'

async function main() {
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

  console.log('\nENRICH Apple Contacts names -> canonical Person:')
  console.log('  enriched with a contact human name:', result.enriched)
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
