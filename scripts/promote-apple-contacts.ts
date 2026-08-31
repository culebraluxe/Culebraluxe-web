#!/usr/bin/env node
// Apple Contacts current ODS -> canonical Person mastering.
// Relationship evidence is retained as provenance/output, not used as a queue.

import { sql } from '../db/client'
import { refreshClientReadModels } from '../db/client-read-models'
import { masterCurrentSourcePeople } from '../db/person-mastering'
import { loadAppleEvidence } from '../lib/relationship-intel/apple-evidence'
import { APPLE_SOURCE } from '../lib/relationship-intel/apple-projector'

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}

async function runMain(): Promise<void> {
  const env = (flag('--env') ?? 'dev').toLowerCase()
  if (env !== 'prod') {
    console.error('Apple Contacts mastering is PROD-only (fail closed)')
    process.exit(2)
  }
  if (!process.env.DATABASE_URL_PROD) {
    console.error('No DATABASE_URL_PROD configured (fail closed)')
    process.exit(2)
  }
  if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
    console.error('PROD mastering selected but the configured connection is the DEV URL (fail closed)')
    process.exit(2)
  }
  process.env.APP_ENV = 'production'

  const accounts = (await sql`
    select distinct source_account
    from l_person
    where source = ${APPLE_SOURCE}
    order by source_account
  `) as { source_account: string }[]
  if (accounts.length !== 1) throw new Error(`expected one current Apple source account; found ${accounts.length}`)
  const sourceAccount = accounts[0].source_account

  const latest = (await sql`
    select id
    from integration_intake_batch
    where source = ${APPLE_SOURCE}
      and source_account = ${sourceAccount}
      and load_status = 'loaded'
    order by received_at desc, created_at desc
    limit 1
  `) as { id: string }[]
  const batchId = latest[0]?.id
  if (!batchId) throw new Error('no loaded Apple Contacts batch exists for current source account')

  const membershipRows = (await sql`
    select count(*)::int as n
    from integration_source_snapshot_member
    where integration_intake_batch_id = ${batchId}
      and source = ${APPLE_SOURCE}
      and source_account = ${sourceAccount}
  `) as { n: number }[]
  const currentRows = (await sql`
    select count(*)::int as n
    from l_person
    where source = ${APPLE_SOURCE}
      and source_account = ${sourceAccount}
  `) as { n: number }[]
  const membership = Number(membershipRows[0]?.n ?? 0)
  const current = Number(currentRows[0]?.n ?? 0)
  if (membership === 0 || current === 0 || membership !== current) {
    throw new Error(`population invariant failed: snapshot=${membership} l_person=${current}`)
  }

  const evidenceProjected = await loadAppleEvidence(sql)
  if (evidenceProjected !== current) {
    throw new Error(`population invariant failed: l_person=${current} evidence=${evidenceProjected}`)
  }

  // Preserve established source ownership from the legacy evidence-backed path
  // before the dedicated source-person link becomes authoritative.
  await sql`
    insert into integration_source_person_link (
      source, source_account, source_identity_key,
      canonical_person_id, link_method, link_reason
    )
    select
      ev.source, ev.source_account, ev.source_identity_key,
      ev.canonical_person_id, 'legacy_evidence_link', 'backfilled_from_relationship_evidence'
    from integration_relationship_evidence ev
    where ev.source = ${APPLE_SOURCE}
      and ev.source_account = ${sourceAccount}
      and ev.canonical_person_id is not null
      and exists (
        select 1 from l_person lp
        where lp.source = ev.source
          and lp.source_account = ev.source_account
          and lp.source_contact_id = ev.source_identity_key
      )
    on conflict (source, source_account, source_identity_key) do nothing
  `

  const conflicts = (await sql`
    select count(*)::int as n
    from integration_source_person_link l
    join integration_relationship_evidence ev
      on ev.source = l.source
     and ev.source_account = l.source_account
     and ev.source_identity_key = l.source_identity_key
    where l.source = ${APPLE_SOURCE}
      and l.source_account = ${sourceAccount}
      and ev.canonical_person_id is not null
      and l.canonical_person_id <> ev.canonical_person_id
      and exists (
        select 1 from l_person lp
        where lp.source = l.source
          and lp.source_account = l.source_account
          and lp.source_contact_id = l.source_identity_key
      )
  `) as { n: number }[]
  const sourceLinkConflicts = Number(conflicts[0]?.n ?? 0)
  if (sourceLinkConflicts > 0) throw new Error(`source ownership conflict before mastering: ${sourceLinkConflicts}`)

  const personBefore = Number(((await sql`select count(*)::int as n from person`) as { n: number }[])[0]?.n ?? 0)
  const identityBefore = Number(((await sql`select count(*)::int as n from person_identity`) as { n: number }[])[0]?.n ?? 0)

  const mastered = await masterCurrentSourcePeople(APPLE_SOURCE)
  if (mastered.current !== current) {
    throw new Error(`mastering input invariant failed: expected ${current}, got ${mastered.current}`)
  }

  await refreshClientReadModels()

  const personAfter = Number(((await sql`select count(*)::int as n from person`) as { n: number }[])[0]?.n ?? 0)
  const identityAfter = Number(((await sql`select count(*)::int as n from person_identity`) as { n: number }[])[0]?.n ?? 0)

  console.log(JSON.stringify({
    env,
    source: APPLE_SOURCE,
    sourceAccount,
    batchId,
    population: {
      snapshotMembers: membership,
      currentLoadPeople: current,
      evidenceProjected,
      masteringInput: mastered.current,
    },
    mastering: mastered,
    canonical: {
      personBefore,
      personAfter,
      personDelta: personAfter - personBefore,
      identityBefore,
      identityAfter,
      identityDelta: identityAfter - identityBefore,
    },
  }, null, 2))
  console.log('[contacts-sync] SUCCESS: current Apple ODS mastered directly into canonical Person; read models refreshed')
}

runMain()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error('[contacts-sync] mastering failed:', (err as Error).message ?? String(err))
    process.exit(1)
  })
