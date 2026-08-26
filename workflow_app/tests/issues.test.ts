// ---------------------------------------------------------------------------
// OPS-11A — Operational Issue Queue: focused unit proofs.
//
//   1. issue runbook config is complete + deterministic responsibility map
//   2. queue read model maps bounded rows (facts + runbook) and is scope/state
//      bounded
//   3. SUPPORT scope yields empty (reusable sibling surface) without hitting
//      the database
//   4. resolve is minimal + idempotent
//   5. /portal/issues belongs to the OPPS operating surface
//
// The deterministic SQL conditions (reconcile) are proven live against the DEV
// control plane by scripts/verify-issues.ts (read-model + dedupe + resolve).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  ISSUE_TYPES,
  RUNBOOK,
  responsibilityForType,
  typesForResponsibility,
} from '../../lib/issue-types'
import { getIssueQueue, resolveIssue } from '../../db/issues'
import { surfaceForPathname } from '../../lib/navigation'

type Captured = { sql: string; params: unknown[] }

function makeExecutor(
  rows: Record<string, unknown>[],
  capture: Captured[],
) {
  return async (
    strings: TemplateStringsArray,
    ...params: unknown[]
  ): Promise<Record<string, unknown>[]> => {
    capture.push({ sql: strings.join('?'), params })
    return rows
  }
}

function cannedRedRow(overrides: Record<string, unknown> = {}) {
  return {
    id: 'issue-1',
    type: 'MISSING_EXECUTED_PS',
    severity: 'RED',
    state: 'OPEN',
    title: 'Purchase agreement not executed',
    detail: 'Deal is under contract without a fully-executed purchase agreement.',
    domain_type: 'deal',
    domain_id: 'deal-1',
    detected_at: '2026-08-25T00:00:00.000Z',
    resolved_at: null,
    related_deal_id: 'deal-1',
    property_name: 'Brisas del Mar',
    client_name: 'Felipe Ortega',
    closing_date: '2026-09-18',
    deal_stage: 'under_contract',
    task_title: null,
    task_due_at: null,
    total: 37,
    ...overrides,
  }
}

test('OPS-11A: every issue type has a runbook with guidance; responsibility map is deterministic', () => {
  for (const type of ISSUE_TYPES) {
    assert.ok(RUNBOOK[type], `runbook exists for ${type}`)
    assert.ok(RUNBOOK[type].steps.length > 0, `${type} runbook has steps`)
    assert.ok(RUNBOOK[type].label.length > 0)
    assert.equal(responsibilityForType(type), 'OPERATIONS_EXCEPTION')
  }
  assert.deepEqual(typesForResponsibility('SUPPORT_EXCEPTION'), [])
  assert.equal(
    responsibilityForType('UNKNOWN_TYPE'),
    'OPERATIONS_EXCEPTION',
    'unknown type still surfaces to a human (fails open to operations)',
  )
})

test('OPS-11A: queue read model maps bounded rows with facts + runbook, paged + scope-filtered', async () => {
  const captured: Captured[] = []
  const fake = makeExecutor(
    [cannedRedRow()],
    captured,
  ) as typeof import('../../db/query-executor').QueryExecutor

  const result = await getIssueQueue(
    { scope: 'OPERATIONS_EXCEPTION', state: 'OPEN', page: 2, pageSize: 10 },
    fake,
  )

  assert.equal(result.rows.length, 1)
  const row = result.rows[0]
  assert.equal(row.severity, 'RED')
  assert.equal(row.typeLabel, 'Purchase Agreement Not Executed')
  assert.equal(row.propertyName, 'Brisas del Mar')
  assert.equal(row.clientName, 'Felipe Ortega')
  assert.equal(row.relatedDealId, 'deal-1')
  assert.equal(row.closingDate, '2026-09-18')
  assert.ok(row.runbook.steps.length > 0, 'runbook attached')
  assert.equal(result.total, 37)
  assert.equal(result.page, 2)

  // Bounded: the generated SQL is LIMIT/OFFSET paged and scope-filtered.
  const text = captured[0].sql.toLowerCase()
  assert.ok(text.includes('limit') && text.includes('offset'), 'bounded page')
  assert.ok(text.includes('state ='), 'state filter applied')
  assert.ok(text.includes('any('), 'scope responsibility filter applied')
})

test('OPS-11A: SUPPORT scope yields empty without touching the database', async () => {
  const throwing = (async () => {
    throw new Error('executor must not be called for an empty scope')
  }) as unknown as typeof import('../../db/query-executor').QueryExecutor
  const result = await getIssueQueue(
    { scope: 'SUPPORT_EXCEPTION', state: 'OPEN' },
    throwing,
  )
  assert.equal(result.rows.length, 0)
  assert.equal(result.total, 0)
})

test('OPS-11A: resolve is minimal + idempotent (only an OPEN row transitions)', async () => {
  const got = await resolveIssue('issue-1', makeExecutor([{ id: 'issue-1' }], []))
  assert.equal(got, true)
  const missed = await resolveIssue('issue-1', makeExecutor([], []))
  assert.equal(missed, false)
})

test('OPS-11A: /portal/issues belongs to the OPPS operating surface', () => {
  assert.equal(surfaceForPathname('/portal/issues'), 'OPS')
  assert.equal(surfaceForPathname('/portal/issues/some-id'), 'OPS')
})
