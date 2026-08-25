// ---------------------------------------------------------------------------
// REL-INTEL — promote review-required ODS evidence into canonical Person rows
// (the "transform into the PARENT" step) so the Clients screen pages the
// canonical `person` table, not the staging tables.
//
// Reads integration_relationship_evidence rows that reconcile to
// `review_required` with a usable phone/email (non-bulk, non-org/service),
// dedupes by primary identity, and creates one canonical Person per identity
// (or links to an existing match). Unmatched / deferred / rejected /
// non_person / ambiguous evidence stays staged. The evidence rows themselves
// are preserved (never deleted) — they are simply marked exact_linked to the
// promoted canonical Person.
//
// DEV only. Never touches production.
//
//   node --env-file=.env.local --import tsx scripts/promote-relationship-evidence.ts
// ---------------------------------------------------------------------------
import { sql } from '../db/client'
import { promoteReviewRequiredEvidence } from '../db/promote-evidence'

async function main() {
  const before = (await sql`select count(*)::int as n from person`) as { n: number }[]
  const evidenceBefore = (await sql`
    select count(*)::int as n from integration_relationship_evidence
  `) as { n: number }[]

  const result = await promoteReviewRequiredEvidence()

  const after = (await sql`select count(*)::int as n from person`) as { n: number }[]
  const evidenceAfter = (await sql`
    select count(*)::int as n from integration_relationship_evidence
  `) as { n: number }[]

  const bySource = (await sql`
    select source, review_state, count(*)::int as n
    from integration_relationship_evidence
    group by source, review_state
    order by source, review_state
  `) as { source: string; review_state: string; n: number }[]

  const linked = (await sql`
    select count(*)::int as n from integration_relationship_evidence
    where canonical_person_id is not null
  `) as { n: number }[]

  console.log('\nPROMOTE -> canonical Person (transform staging -> parent):')
  console.log('  created persons:', result.created, '| linked existing:', result.linkedExisting)
  console.log('  identity groups:', result.groups, '| evidence rows linked:', result.evidenceLinked)
  console.log('  person rows before:', before[0].n, '-> after:', after[0].n)
  console.log('  evidence rows preserved before:', evidenceBefore[0].n, '-> after:', evidenceAfter[0].n)
  console.log('  evidence linked to a canonical Person:', linked[0].n)
  console.log('\n  evidence by source/review_state after promotion:')
  for (const row of bySource) {
    console.log(`    ${row.source} · ${row.review_state}: ${row.n}`)
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })
