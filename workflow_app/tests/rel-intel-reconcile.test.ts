import { test } from 'node:test'
import assert from 'node:assert/strict'

import type {
  RelationshipEvidence,
  ReviewState,
} from '../../lib/relationship-intel/contracts'
import {
  reconcileEvidence,
  reconcileEvidenceBatch,
  type PersonLookup,
} from '../../lib/relationship-intel/reconcile'

// ---------------------------------------------------------------------------
// REL-INTEL — deterministic reconciliation. No database.
// ---------------------------------------------------------------------------

function evidence(overrides: Partial<RelationshipEvidence> = {}): RelationshipEvidence {
  return {
    source: 'gmail_contacts',
    sourceAccount: 'acc',
    sourceIdentityKey: 'jane@example.com',
    emails: [{ value: 'jane@example.com', normalized: 'jane@example.com', label: null }],
    phones: [],
    hasEmail: true,
    hasPhone: false,
    isTwoWay: true,
    isOwnerInitiated: false,
    isAutomatedOrBulk: false,
    isOrganizationOrService: false,
    ...overrides,
  }
}

function lookup(overrides: Partial<PersonLookup> = {}): PersonLookup {
  return {
    findExplicitSourceLink: async () => null,
    findPeopleByEmail: async () => [],
    findPeopleByPhone: async () => [],
    ...overrides,
  }
}

test('REL-INTEL: exact email match links exact', async () => {
  const d = await reconcileEvidence(
    evidence(),
    lookup({
      findPeopleByEmail: async () => [{ personId: 'person-1' }],
    }),
  )
  assert.equal(d.reviewState, 'exact_linked')
  assert.equal(d.matchMethod, 'exact_email')
  assert.equal(d.canonicalPersonId, 'person-1')
})

test('REL-INTEL: exact phone match links exact', async () => {
  const d = await reconcileEvidence(
    evidence({
      emails: [],
      phones: [{ value: '+1 (787) 555-0134', normalized: '7875550134', label: null }],
      hasPhone: true,
    }),
    lookup({
      findPeopleByPhone: async () => [{ personId: 'person-2' }],
    }),
  )
  assert.equal(d.reviewState, 'exact_linked')
  assert.equal(d.matchMethod, 'exact_phone')
})

test('REL-INTEL: explicit source link wins', async () => {
  const d = await reconcileEvidence(
    evidence(),
    lookup({
      findExplicitSourceLink: async () => ({ personId: 'person-3' }),
      findPeopleByEmail: async () => [{ personId: 'person-4' }],
    }),
  )
  assert.equal(d.reviewState, 'exact_linked')
  assert.equal(d.matchMethod, 'source_link')
  assert.equal(d.canonicalPersonId, 'person-3')
})

test('REL-INTEL: conflicting email matches produce an ambiguous review', async () => {
  const d = await reconcileEvidence(
    evidence(),
    lookup({
      findPeopleByEmail: async () => [{ personId: 'p1' }, { personId: 'p2' }],
    }),
  )
  assert.equal(d.reviewState, 'ambiguous')
  assert.equal(d.canonicalPersonId, null)
})

test('REL-INTEL: fuzzy name alone never auto-matches', async () => {
  const d = await reconcileEvidence(
    evidence({
      emails: [],
      phones: [],
      hasEmail: false,
      hasPhone: false,
      displayName: 'Jane Q. Doe',
    }),
    lookup({
      findPeopleByEmail: async () => [],
      findPeopleByPhone: async () => [],
    }),
  )
  // No usable identity + no exact match -> deferred, not silently linked.
  assert.equal(d.reviewState, 'deferred')
  assert.equal(d.canonicalPersonId, null)
})

test('REL-INTEL: automated/bulk senders are suppressed (not made clients)', async () => {
  const d = await reconcileEvidence(
    evidence({ isAutomatedOrBulk: true }),
    lookup({ findPeopleByEmail: async () => [{ personId: 'p1' }] }),
  )
  assert.equal(d.reviewState, 'rejected')
  assert.equal(d.canonicalPersonId, null)
})

test('REL-INTEL: service/organization evidence is non_person', async () => {
  const d = await reconcileEvidence(
    evidence({ isOrganizationOrService: true }),
    lookup({ findPeopleByEmail: async () => [{ personId: 'p1' }] }),
  )
  assert.equal(d.reviewState, 'non_person')
})

test('REL-INTEL: meaningful unmatched evidence is a review candidate', async () => {
  const d = await reconcileEvidence(
    evidence({ isTwoWay: true, isOwnerInitiated: true }),
    lookup(),
  )
  assert.equal(d.reviewState, 'review_required')
  assert.equal(d.matchConfidence, 'probable')
})

test('REL-INTEL: ordinary unmatched correspondent stays unmatched', async () => {
  const d = await reconcileEvidence(
    evidence({ isTwoWay: false, isOwnerInitiated: false, outboundCount: 0 }),
    lookup(),
  )
  assert.equal(d.reviewState, 'unmatched')
})

test('REL-INTEL: reconciliation is deterministic and rerunnable', async () => {
  const rows = [evidence(), evidence({ emails: [], hasEmail: false, hasPhone: false })]
  const lk = lookup({ findPeopleByEmail: async () => [{ personId: 'p1' }] })
  const first = await reconcileEvidenceBatch(rows, lk)
  const second = await reconcileEvidenceBatch(rows, lk)
  const states1 = [...first.values()].map((d) => d.reviewState).sort()
  const states2 = [...second.values()].map((d) => d.reviewState).sort()
  assert.deepEqual(states1, states2)
  assert.ok(states1.every((s) => typeof s === 'string'))
  // Every decision carries the rule version.
  for (const d of first.values()) assert.equal(d.ruleVersion, 'rel-intel/v1')
})

test('REL-INTEL: evidence with a review outcome type is well-formed', async () => {
  const outcomes: ReviewState[] = [
    'exact_linked',
    'review_required',
    'ambiguous',
    'unmatched',
    'rejected',
    'non_person',
    'deferred',
  ]
  const d = await reconcileEvidence(
    evidence(),
    lookup({ findPeopleByEmail: async () => [{ personId: 'p1' }] }),
  )
  assert.ok(outcomes.includes(d.reviewState))
})
