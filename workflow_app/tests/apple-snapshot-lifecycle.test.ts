import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { phoneDigitsKey, createInMemoryPersonLookup } from '../../lib/relationship-intel/inmemory-lookup'
import { reconcileEvidence } from '../../lib/relationship-intel/reconcile'

// ---------------------------------------------------------------------------
// APPLE CONTACTS — current-snapshot lifecycle invariant.
//
// ODS history grows (A,B,C,D) but the CURRENT snapshot / l_person / current
// reconciliation population is exactly the current snapshot members (B,C,D).
// A replay is a current member even when its latest staged revision belongs to an
// older batch. Historical source identities never re-enter the current pipeline.
// ---------------------------------------------------------------------------

const LOADER = readFileSync('scripts/load-apple-contacts.ts', 'utf8')
const PROJECTOR = readFileSync('scripts/project-apple-contacts.ts', 'utf8')
const MIGRATION = readFileSync('db/migrations/096_integration_source_snapshot_member.sql', 'utf8')

type Staged = { key: string; rev: number; batch: string }

/** Pure model of the projection's "latest staged revision per CURRENT member". */
function currentProjection(staged: Staged[], snapshotMembers: string[]): string[] {
  const memberSet = new Set(snapshotMembers)
  const byKey = new Map<string, Staged>()
  for (const s of staged) {
    const existing = byKey.get(s.key)
    if (!existing || s.rev > existing.rev) byKey.set(s.key, s)
  }
  // Current members, latest staged revision regardless of which batch created it.
  return snapshotMembers
    .filter((key) => memberSet.has(key) && byKey.has(key))
    .sort()
}

test('lifecycle A+B: Snapshot A then Snapshot B -> l_person is exactly B,C,D; A stays in ODS', () => {
  const staged: Staged[] = [
    { key: 'A', rev: 1, batch: 'X' },
    { key: 'B', rev: 1, batch: 'X' },
    { key: 'C', rev: 1, batch: 'X' },
    { key: 'D', rev: 1, batch: 'Y' },
  ]
  const membershipX = ['A', 'B', 'C']
  const membershipY = ['B', 'C', 'D']

  const before = currentProjection(staged, membershipX)
  assert.deepEqual(before, ['A', 'B', 'C'], 'Snapshot A projected A,B,C')

  const after = currentProjection(staged, membershipY)
  assert.deepEqual(after, ['B', 'C', 'D'], 'Snapshot B projected B,C,D')
  assert.ok(staged.some((s) => s.key === 'A'), 'A remains preserved in ODS history')
})

test('lifecycle C: replay member uses its LATEST staged profile even if created by an older batch', () => {
  const staged: Staged[] = [
    { key: 'B', rev: 1, batch: 'X' },
    { key: 'B', rev: 2, batch: 'X' },
  ]
  const proj = currentProjection(staged, ['B'])
  assert.deepEqual(proj, ['B'], 'B remains current')
  const latestRev = Math.max(...staged.filter((s) => s.key === 'B').map((s) => s.rev))
  assert.equal(latestRev, 2, 'B uses its latest staged revision')
})

test('lifecycle D: rerunning Snapshot B is stable (no growth, no re-entry of A)', () => {
  const staged: Staged[] = [
    { key: 'A', rev: 1, batch: 'X' },
    { key: 'B', rev: 1, batch: 'X' },
    { key: 'C', rev: 1, batch: 'X' },
    { key: 'D', rev: 1, batch: 'Y' },
  ]
  const first = currentProjection(staged, ['B', 'C', 'D'])
  const second = currentProjection(staged, ['B', 'C', 'D'])
  assert.deepEqual(first, second, 'stable across reruns')
  assert.ok(!first.includes('A'), 'historical A never re-enters the current pipeline')
})

test('lifecycle E: membership is recorded for EVERY export contact, INCLUDING exact replays', () => {
  assert.ok(LOADER.includes('prepared.map((p) => p.sourceId)'), 'membership derives from ALL export contacts')
  assert.ok(
    LOADER.indexOf('insert into integration_source_snapshot_member') < LOADER.indexOf('load_status = ${loadStatus}'),
    'membership is recorded BEFORE the batch is marked loaded',
  )
  assert.ok(
    LOADER.includes('on conflict (integration_intake_batch_id, source, source_account, source_identity_key) do nothing'),
    'idempotent membership insert',
  )
})

test('lifecycle F: projection restricts l_person to current snapshot members and prunes non-members', () => {
  assert.ok(PROJECTOR.includes('from integration_source_snapshot_member m'), 'projection reads current membership')
  assert.ok(PROJECTOR.includes('join current_snapshot cs'), 'latest staged is restricted to current members')
  assert.ok(PROJECTOR.includes('delete from l_person lp'), 'non-current l_person rows are pruned')
  assert.ok(PROJECTOR.includes("load_status = 'loaded'"), 'projection uses the latest LOADED batch')
})

test('lifecycle G: operator masters exactly the current l_person population', () => {
  const promote = readFileSync('db/promote-evidence.ts', 'utf8')
  assert.ok(promote.includes('lp.source_contact_id = integration_relationship_evidence.source_identity_key'), 'legacy evidence promotion can restrict to current l_person members')
  const run = readFileSync('scripts/promote-apple-contacts.ts', 'utf8')
  assert.ok(run.includes('loadAppleEvidence'), 'operator projects current Apple evidence before mastering')
  assert.ok(run.includes('masterCurrentSourcePeople'), 'operator masters current source people directly')
  assert.ok(run.includes('membership !== current'), 'operator fails closed if snapshot and l_person populations diverge')
  assert.ok(run.includes('mastered.current !== current'), 'operator verifies mastering consumed the current population')
})

test('lifecycle H: migration enforces unique membership per batch + FK', () => {
  assert.ok(MIGRATION.includes('create table integration_source_snapshot_member'), 'membership table created')
  assert.ok(MIGRATION.includes('unique (integration_intake_batch_id, source, source_account, source_identity_key)'), 'no duplicate membership per batch')
  assert.ok(MIGRATION.includes('references integration_intake_batch'), 'FK to the intake batch')
})

test('lifecycle I: NANP E.164 and ten-digit handles share one semantic phone key', () => {
  assert.equal(phoneDigitsKey('+8609895020'), '8609895020')
  assert.equal(phoneDigitsKey('8609895020'), '8609895020')
  assert.equal(phoneDigitsKey('+8609895020'), phoneDigitsKey('8609895020'), '+8609895020 == 8609895020')
  assert.equal(phoneDigitsKey('+18609895020'), '8609895020', 'leading NANP country code 1 is stripped for semantic matching')
  assert.equal(phoneDigitsKey('+18609895020'), phoneDigitsKey('8609895020'), 'E.164 and ten-digit NANP representations match')
  assert.equal(phoneDigitsKey('+442079460958'), '442079460958', 'non-NANP country codes are never stripped')
})

test('lifecycle J: multi-owner phone conflict is surfaced (not silently first-won)', async () => {
  const identityRows = [
    { identity_type: 'phone', identity_value: '+8609895020', person_id: 'p-a' },
    { identity_type: 'phone', identity_value: '8609895020', person_id: 'p-b' },
  ]
  const execute = ((_s: TemplateStringsArray) => Promise.resolve(identityRows)) as unknown as Parameters<typeof createInMemoryPersonLookup>[0]
  const { lookup } = await createInMemoryPersonLookup(execute)
  const people = await lookup.findPeopleByPhone('+8609895020')
  assert.deepEqual(people.map((p) => p.personId).sort(), ['p-a', 'p-b'], 'BOTH owners returned -> ambiguous, never first-won')
})

test('lifecycle K: reconcile marks a multi-owner phone conflict ambiguous', async () => {
  const evidence = {
    source: 'apple_contacts', sourceAccount: 'acc', sourceIdentityKey: 'X:ABPerson', displayName: 'Ami',
    organization: null, emails: [], phones: [{ value: '+8609895020', normalized: '+8609895020', label: null }],
    hasEmail: false, hasPhone: true, isAutomatedOrBulk: false, isOrganizationOrService: false,
  } as never
  const decision = await reconcileEvidence(evidence, {
    findExplicitSourceLink: async () => null,
    findPeopleByEmail: async () => [],
    findPeopleByPhone: async () => [{ personId: 'p-a' }, { personId: 'p-b' }],
  })
  assert.equal(decision.reviewState, 'ambiguous', 'multi-owner phone -> ambiguous, not silent')
  assert.equal(decision.canonicalPersonId, null)
})
