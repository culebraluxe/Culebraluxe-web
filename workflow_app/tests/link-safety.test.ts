import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { mergeReconcileDecision } from '../../lib/relationship-intel/link-safety'
import {
  normalizeEvidenceIdentities,
  planIdentityAttachments,
} from '../../db/promote-evidence'
import type { ReconcileDecision } from '../../lib/relationship-intel/contracts'

// ---------------------------------------------------------------------------
// REL-INTEL — match-once / enrich-forever canonical link safety.
//
// Once a source identity has a canonical_person_id, that Person is its durable
// owner. Automated reconciliation may ENRICH it but never re-decides who it is:
// no clearing, no redirecting, no merging. These tests lock that invariant into
// the write seam's pure decision merge.
// ---------------------------------------------------------------------------

function decision(overrides: Partial<ReconcileDecision> = {}): ReconcileDecision {
  return {
    reviewState: 'exact_linked',
    matchMethod: 'exact_email',
    matchConfidence: 'exact',
    canonicalPersonId: null,
    reason: 'exact_normalized_identity',
    ruleVersion: 'rel-intel/v1',
    ...overrides,
  }
}

const IDENTITIES = normalizeEvidenceIdentities(
  [{ value: 'a@example.com', normalized: 'a@example.com' }],
  [{ value: '+17875550101', normalized: '17875550101' }],
)

test('link-safety 1: existing source link + exact replay preserves the SAME Person', () => {
  const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-a' }))
  assert.equal(write.canonicalPersonId, 'p-a')
  assert.equal(write.reviewState, 'exact_linked')
  assert.equal(write.conflictSurfaced, false)
})

test('link-safety 2: existing source link + changed evidence fingerprint preserves the SAME Person', () => {
  const write = mergeReconcileDecision(
    'p-a',
    decision({ canonicalPersonId: 'p-a', reason: 'changed_fingerprint_replay' }),
  )
  assert.equal(write.canonicalPersonId, 'p-a')
  assert.equal(write.conflictSurfaced, false)
})

test('link-safety 3: existing source link + new unused email attaches to the SAME Person', () => {
  // email unused, phone already owned by the established Person.
  const plan = planIdentityAttachments(IDENTITIES, [null, 'p-a'], 'p-a')
  assert.equal(plan.attach.length, 1, 'new unused email attaches')
  assert.equal(plan.attach[0].kind, 'email')
  assert.equal(plan.conflicts.length, 0)
})

test('link-safety 4: existing source link + new unused phone attaches to the SAME Person', () => {
  // email already owned, phone unused.
  const plan = planIdentityAttachments(IDENTITIES, ['p-a', null], 'p-a')
  assert.equal(plan.attach.length, 1, 'new unused phone attaches')
  assert.equal(plan.attach[0].kind, 'phone')
  assert.equal(plan.conflicts.length, 0)
})

test('link-safety 5: identity already owned by the same Person is a replay/no-op', () => {
  const plan = planIdentityAttachments(IDENTITIES, ['p-a', 'p-a'], 'p-a')
  assert.equal(plan.duplicate, 2, 'both identities already owned -> no-op')
  assert.equal(plan.attach.length, 0)
  assert.equal(plan.conflicts.length, 0)
})

test('link-safety 6: identity owned by a different Person preserves the link, moves nothing, merges nothing', () => {
  // Link seam: established p-a, automated decision suggests p-b -> preserve p-a.
  const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-b' }))
  assert.equal(write.canonicalPersonId, 'p-a', 'established link preserved')
  assert.equal(write.conflictSurfaced, true, 'conflict surfaced')

  // Enrichment seam: phone owned by p-b while target is p-a -> never moved/merged.
  const plan = planIdentityAttachments(IDENTITIES, ['p-a', 'p-b'], 'p-a')
  assert.equal(plan.conflicts.length, 1)
  assert.equal(plan.conflicts[0].personId, 'p-b')
  assert.equal(plan.attach.length, 0, 'conflicting identity not moved to p-a')
})

test('link-safety 7: later reconciliation returning NULL leaves the established link unchanged', () => {
  const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: null, reviewState: 'unmatched' }))
  assert.equal(write.canonicalPersonId, 'p-a', 'never cleared')
  assert.equal(write.reviewState, 'exact_linked')
  assert.equal(write.conflictSurfaced, true)
})

test('link-safety 8: later automated reconciliation suggesting another Person is ignored', () => {
  const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-b', reviewState: 'exact_linked' }))
  assert.equal(write.canonicalPersonId, 'p-a', 'never redirected to p-b')
  assert.equal(write.conflictSurfaced, true)
})

test('link-safety 9: classification changes later to ambiguous/rejected/non_person do NOT destroy the link', () => {
  for (const state of ['ambiguous', 'rejected', 'non_person'] as const) {
    const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: null, reviewState: state }))
    assert.equal(write.canonicalPersonId, 'p-a', `${state} must not clear the link`)
    assert.equal(write.reviewState, 'exact_linked')
    assert.equal(write.conflictSurfaced, true)
  }
})

test('link-safety 10: existing buyer/seller/both/unclassified role survives enrichment unchanged', () => {
  const src = readFileSync('db/promote-evidence.ts', 'utf8')
  assert.ok(!src.includes('set role'), 'promotion/enrichment never issues a role UPDATE')
  assert.ok(src.includes("role: 'unclassified'"), 'new Persons are created unclassified (roles preserved for existing)')
})

test('link-safety 11: genuinely unlinked evidence still follows the normal reconciliation path', () => {
  const establish = mergeReconcileDecision(null, decision({ canonicalPersonId: 'p-a' }))
  assert.equal(establish.canonicalPersonId, 'p-a', 'establish once when no link exists')
  assert.equal(establish.conflictSurfaced, false)

  const staysNull = mergeReconcileDecision(null, decision({ canonicalPersonId: null, reviewState: 'unmatched' }))
  assert.equal(staysNull.canonicalPersonId, null)
  assert.equal(staysNull.reviewState, 'unmatched')
})

