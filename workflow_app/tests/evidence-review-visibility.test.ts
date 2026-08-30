import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { getRelationshipEvidenceReview } from '../../db/relationship-evidence'
import { mergeReconcileDecision } from '../../lib/relationship-intel/link-safety'
import type { ReconcileDecision } from '../../lib/relationship-intel/contracts'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// REL-INTEL — established-link conflict stewardship visibility.
//
// A preserved-link conflict keeps the source durably linked to its Person
// (review_state = exact_linked, canonical_person_id = Person A) while marking
// the automated conflict (match_confidence = ambiguous, match_reason =
// established_link_preserved_automated_conflict). This file proves that such a
// row REMAINS VISIBLE to the existing relationship-evidence stewardship query —
// it is NOT hidden by treating review_state = exact_linked as fully clean.
// ---------------------------------------------------------------------------

const CONFLICT_REASON = 'established_link_preserved_automated_conflict'

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

/** A raw integration_relationship_evidence DB row (shape mapRow consumes). */
function rawRow(overrides: Record<string, unknown> = {}) {
  return {
    id: 'ev-1', source: 'apple_messages', source_account: 'acc', source_identity_key: 'handle-1',
    source_label: null, display_name: 'Person A', organization: null, emails: [], phones: [],
    first_observed_at: null, last_observed_at: '2026-08-28T14:52:25.245Z',
    last_inbound_at: null, last_outbound_at: null,
    inbound_count: null, outbound_count: null, is_two_way: null, is_owner_initiated: null,
    is_automated_or_bulk: null, is_organization_or_service: null, known_apple_contact: null,
    has_email: false, has_phone: true, coverage_note: null,
    canonical_person_id: 'p-a', match_method: 'exact_email', match_confidence: 'ambiguous',
    review_state: 'exact_linked', match_reason: CONFLICT_REASON,
    rule_version: 'rel-intel/v1', evidence_fingerprint: 'fp', updated_at: '2026-08-28T14:52:25.245Z',
    ...overrides,
  }
}

test('visibility 1: established-link conflict preserves canonical_person_id (never cleared/redirected)', () => {
  const clear = mergeReconcileDecision('p-a', decision({ canonicalPersonId: null, reviewState: 'unmatched' }))
  assert.equal(clear.canonicalPersonId, 'p-a', 'never cleared')
  const redirect = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-b' }))
  assert.equal(redirect.canonicalPersonId, 'p-a', 'never redirected')
  const same = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-a' }))
  assert.equal(same.canonicalPersonId, 'p-a')
})

test('visibility 2: preserved-link conflict appears in the existing stewardship/review result', async () => {
  const rows = [rawRow()]
  const execute = ((_strings: TemplateStringsArray) => Promise.resolve(rows)) as unknown as QueryExecutor

  const all = await getRelationshipEvidenceReview({ reviewState: 'all' }, execute)
  assert.equal(all.rows.length, 1)
  const row = all.rows[0]
  assert.equal(row.reviewState, 'exact_linked')
  assert.equal(row.canonicalPersonId, 'p-a')
  assert.equal(row.matchConfidence, 'ambiguous')
  assert.equal(row.matchReason, CONFLICT_REASON)

  // Even when an operator filters the stewardship surface to the exact_linked
  // bucket, the conflict row is returned (it is NOT filtered out as "clean").
  const exactBucket = await getRelationshipEvidenceReview({ reviewState: 'exact_linked' }, execute)
  assert.ok(exactBucket.rows.some((r) => r.matchReason === CONFLICT_REASON))
})

test('visibility 3: ordinary clean exact_linked row is NOT a false conflict', async () => {
  const conflict = rawRow()
  const clean = rawRow({ id: 'ev-2', match_confidence: 'exact', match_reason: 'exact_normalized_identity' })
  const execute = ((_strings: TemplateStringsArray) => Promise.resolve([conflict, clean])) as unknown as QueryExecutor
  const res = await getRelationshipEvidenceReview({ reviewState: 'exact_linked' }, execute)

  const cleanRow = res.rows.find((r) => r.id === 'ev-2')
  assert.ok(cleanRow, 'clean row returned')
  assert.equal(cleanRow.matchConfidence, 'exact', 'not flagged ambiguous')
  assert.notEqual(cleanRow.matchReason, CONFLICT_REASON, 'not a false conflict')

  // Pure merge agrees: same-person replay is a clean preserve, no conflict surfaced.
  const same = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-a' }))
  assert.equal(same.conflictSurfaced, false)
  assert.equal(same.matchConfidence, 'exact')
})

test('visibility 4: ambiguous/unmatched/review-required behavior is unchanged', () => {
  for (const state of ['ambiguous', 'unmatched', 'review_required'] as const) {
    const write = mergeReconcileDecision(null, decision({
      canonicalPersonId: null,
      reviewState: state,
      matchConfidence: state === 'review_required' ? 'probable' : 'none',
    }))
    assert.equal(write.reviewState, state, `${state} passes through unchanged`)
    assert.equal(write.canonicalPersonId, null)
    assert.equal(write.conflictSurfaced, false)
  }
})

test('visibility 5: Client relationship source state stays available after a preserved-link conflict', () => {
  // A preserved conflict keeps review_state = exact_linked and canonical_person_id
  // = Person A, which is exactly the predicate mv_client_relationship_channels
  // uses — so the source remains in the Client relationship model.
  const write = mergeReconcileDecision('p-a', decision({ canonicalPersonId: 'p-b' }))
  assert.equal(write.reviewState, 'exact_linked')
  assert.equal(write.canonicalPersonId, 'p-a')

  const mig = readFileSync('db/migrations/094_mv_client_relationship_channels.sql', 'utf8')
  assert.ok(mig.includes("ev.review_state = 'exact_linked'"), 'channels MV includes exact_linked rows')
  assert.ok(mig.includes('ev.canonical_person_id is not null'), 'channels MV requires the durable link')
})

