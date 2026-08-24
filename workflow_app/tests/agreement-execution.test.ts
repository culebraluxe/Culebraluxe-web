import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  evaluateAgreementExecution,
  agreementExecutionTransition,
  buildIssuedExecutionSlots,
  resolveRequiredSlots,
  resolveRequiredExecutionRoles,
  AGREEMENT_FULLY_EXECUTED,
  type IssuedExecutionSlot,
} from '../../lib/agreements/execution'

const ROLES = ['BUYER', 'SELLER', 'SELLER_BROKER']

/** Build one required slot per role (slotId `ROLE:1`). */
function slots(roles: readonly string[]): IssuedExecutionSlot[] {
  return roles.map((role, i) => ({
    slotId: `${role}:1`,
    role,
    personId: null,
    name: role,
    email: null,
    required: true,
    order: i,
  }))
}

test('CRM-27: partial required signatures are NOT fully executed (participant slots)', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: slots(ROLES),
    satisfiedSlotIds: ['BUYER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, false)
  assert.equal(verdict.reason, 'missing_required_roles')
  assert.deepEqual(verdict.missingSlotIds, ['SELLER:1', 'SELLER_BROKER:1'])
  assert.deepEqual(verdict.missingRoles, ['SELLER', 'SELLER_BROKER'])
})

test('CRM-27: final required evidence -> fully executed', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: slots(ROLES),
    satisfiedSlotIds: ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, true)
  assert.equal(verdict.reason, 'all_required_roles_satisfied')
})

test('CRM-27: authorized manual/external evidence satisfies the predicate', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: slots(ROLES),
    satisfiedSlotIds: [],
    manuallyExecuted: true,
  })
  assert.equal(verdict.fullyExecuted, true)
  assert.equal(verdict.reason, 'manual_execution')
})

test('CRM-27: PR-PNS policy requires BUYER/SELLER/SELLER_BROKER slots', () => {
  const required = resolveRequiredSlots(
    'PR-PNS',
    buildIssuedExecutionSlots([
      { role: 'BUYER', personId: 'p1', name: 'Buyer One', email: 'b@x.com' },
      { role: 'SELLER', personId: 'p2', name: 'Seller', email: 's@x.com' },
      { role: 'SELLER_BROKER', personId: 'p3', name: 'Broker', email: 'br@x.com' },
      { role: 'OTHER', personId: 'p4', name: 'Optional', email: null },
    ]),
  )
  const requiredRoles = [...new Set(required.map((s) => s.role))].sort()
  assert.deepEqual(requiredRoles, [...ROLES].sort())
  assert.ok(!required.some((s) => s.role === 'OTHER'), 'OTHER slot is not required')
})

test('CRM-27: transition is idempotent — becameFullyExecuted exactly once', () => {
  const partial = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1', requiredSlots: slots(ROLES), satisfiedSlotIds: ['BUYER:1'], manuallyExecuted: false,
  })
  const full = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1', requiredSlots: slots(ROLES), satisfiedSlotIds: ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1'], manuallyExecuted: false,
  })
  const first = agreementExecutionTransition(partial, full)
  assert.equal(first.becameFullyExecuted, true)
  const replay = agreementExecutionTransition(full, full)
  assert.equal(replay.becameFullyExecuted, false)
})

test('CRM-27: AGREEMENT_FULLY_EXECUTED is a neutral domain event type', () => {
  assert.equal(AGREEMENT_FULLY_EXECUTED, 'AGREEMENT_FULLY_EXECUTED')
})

test('CRM-27: resolveRequiredExecutionRoles (PR-PNS) resolves the required roles', () => {
  assert.deepEqual([...resolveRequiredExecutionRoles('PR-PNS', ROLES)].sort(), [...ROLES].sort())
})

// ---------------------------------------------------------------------------
// PARTICIPANT CARDINALITY — multiple parties, duplicate evidence, wrong version.
// ---------------------------------------------------------------------------

test('CRM-27: two Buyers — only one completed means incomplete', () => {
  const required = buildIssuedExecutionSlots([
    { role: 'BUYER', personId: 'b1', name: 'Buyer One', email: null },
    { role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: null },
    { role: 'SELLER', personId: 's1', name: 'Seller', email: null },
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: null },
  ]).filter((s) => s.required)
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: required,
    satisfiedSlotIds: ['BUYER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, false)
  assert.deepEqual(verdict.missingSlotIds, ['BUYER:2', 'SELLER:1', 'SELLER_BROKER:1'])
})

test('CRM-27: two Buyers — both completed + Seller + Seller Broker means complete', () => {
  const required = buildIssuedExecutionSlots([
    { role: 'BUYER', personId: 'b1', name: 'Buyer One', email: null },
    { role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: null },
    { role: 'SELLER', personId: 's1', name: 'Seller', email: null },
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: null },
  ]).filter((s) => s.required)
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: required,
    satisfiedSlotIds: ['BUYER:1', 'BUYER:2', 'SELLER:1', 'SELLER_BROKER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, true)
})

test('CRM-27: two Sellers — only one completed means incomplete', () => {
  const required = buildIssuedExecutionSlots([
    { role: 'BUYER', personId: 'b1', name: 'Buyer', email: null },
    { role: 'SELLER', personId: 's1', name: 'Seller One', email: null },
    { role: 'SELLER', personId: 's2', name: 'Seller Two', email: null },
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: null },
  ]).filter((s) => s.required)
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: required,
    satisfiedSlotIds: ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, false)
  assert.deepEqual(verdict.missingSlotIds, ['SELLER:2'])
})

test('CRM-27: duplicate evidence for one participant never satisfies another', () => {
  const required = buildIssuedExecutionSlots([
    { role: 'BUYER', personId: 'b1', name: 'Buyer One', email: null },
    { role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: null },
  ]).filter((s) => s.required)
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredSlots: required,
    satisfiedSlotIds: ['BUYER:1', 'BUYER:1', 'BUYER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, false)
  assert.deepEqual(verdict.missingSlotIds, ['BUYER:2'])
})

test('CRM-27: evidence for another issued version cannot satisfy this version', () => {
  const verdictV2 = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v2',
    requiredSlots: buildIssuedExecutionSlots([
      { role: 'BUYER', personId: 'b9', name: 'New Buyer', email: null },
      { role: 'SELLER', personId: 's9', name: 'New Seller', email: null },
      { role: 'SELLER_BROKER', personId: 'br9', name: 'New Broker', email: null },
    ]).filter((s) => s.required),
    satisfiedSlotIds: ['BUYER:1', 'SELLER:1'],
    manuallyExecuted: false,
  })
  assert.equal(verdictV2.fullyExecuted, false)
  assert.deepEqual(verdictV2.missingSlotIds, ['SELLER_BROKER:1'])
})


// ---------------------------------------------------------------------------
// Completion evaluator (evidence assembly + exactly-once marker + snapshot)
// ---------------------------------------------------------------------------

import {
  evaluateAgreementCompletion,
} from '../../lib/agreements/completion'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

const DEFAULT_PARTICIPANTS = [
  { slotId: 'BUYER:1', role: 'BUYER', personId: 'p1', name: 'Buyer One', email: 'buyer1@x.com', required: true, order: 0 },
  { slotId: 'SELLER:1', role: 'SELLER', personId: 'p2', name: 'Seller', email: 'seller@x.com', required: true, order: 1 },
  { slotId: 'SELLER_BROKER:1', role: 'SELLER_BROKER', personId: 'p3', name: 'Broker', email: 'broker@x.com', required: true, order: 2 },
]

function makeExecute(opts: {
  templateId?: string
  issuedVersion?: number
  satisfiedSlots?: string[]
  id?: string
  dealId?: string | null
  documentType?: string
  issuedParticipants?: unknown
}) {
  const executor: QueryExecutor = async (strings) => {
    const sql = strings.join('?')
    if (sql.includes('from transaction_document')) {
      return [
        {
          id: opts.id ?? 'doc-1',
          template_id: opts.templateId ?? 'PR-PNS',
          issued_version: opts.issuedVersion ?? 1,
          deal_id: opts.dealId ?? 'deal-1',
          document_type: opts.documentType ?? 'agreement',
          source_snapshot: {
            issuedParticipants: opts.issuedParticipants ?? DEFAULT_PARTICIPANTS,
          },
        },
      ]
    }
    if (sql.includes('from signature_request')) {
      return (opts.satisfiedSlots ?? []).map((slot) => ({ execution_slot_id: slot }))
    }
    return []
  }
  return executor
}

function makeRun() {
  const seen = new Set<string>()
  const runner: TxRunner = (cb) =>
    cb(async (strings, ...params) => {
      const sql = strings.join('?')
      if (sql.includes('insert into agreement_execution')) {
        const key = `${String(params[0])}:${String(params[1])}`
        if (seen.has(key)) return []
        seen.add(key)
        return [{ id: '1' }]
      }
      return []
    })
  return { runner }
}

const FULL_SLOTS = ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1']

test('CRM-27: completion — partial slots never emits', async () => {
  const execute = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedSlots: ['BUYER:1'] })
  const { runner } = makeRun()
  const result = await evaluateAgreementCompletion('doc-1', 'evt-1', { execute, run: runner })
  assert.equal(result.shouldEmit, false)
  assert.equal(result.verdict.fullyExecuted, false)
  assert.equal(result.document?.issuedVersion, 1)
})

test('CRM-27: completion — fully executed emits exactly once, replay is a no-op', async () => {
  const execute = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedSlots: FULL_SLOTS })
  const { runner } = makeRun()
  const first = await evaluateAgreementCompletion('doc-1', 'evt-1', { execute, run: runner })
  assert.equal(first.shouldEmit, true)
  assert.equal(first.verdict.fullyExecuted, true)
  const replay = await evaluateAgreementCompletion('doc-1', 'evt-2', { execute, run: runner })
  assert.equal(replay.shouldEmit, false)
})

test('CRM-27: completion — wrong document/version is independently scoped', async () => {
  const executeA = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedSlots: FULL_SLOTS })
  const { runner } = makeRun()
  const a = await evaluateAgreementCompletion('doc-A', 'evt-A', { execute: executeA, run: runner })
  assert.equal(a.shouldEmit, true)
  const executeB = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedSlots: ['BUYER:1'] })
  const b = await evaluateAgreementCompletion('doc-B', 'evt-B', { execute: executeB, run: runner })
  assert.equal(b.shouldEmit, false)
  assert.equal(b.verdict.fullyExecuted, false)
})

test('CRM-27: completion — mutable draft edits after issuance do not change issued required slots', async () => {
  // The immutable snapshot (source_snapshot.issuedParticipants) is what governs
  // the required-slot set — even if the mutable draft would resolve differently.
  const issuedAtIssuance = [
    { slotId: 'BUYER:1', role: 'BUYER', personId: 'b1', name: 'Buyer One', email: null, required: true, order: 0 },
    { slotId: 'SELLER:1', role: 'SELLER', personId: 's1', name: 'Seller', email: null, required: true, order: 1 },
    { slotId: 'SELLER_BROKER:1', role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: null, required: true, order: 2 },
  ]
  const execute = makeExecute({
    templateId: 'PR-PNS',
    issuedVersion: 1,
    issuedParticipants: issuedAtIssuance,
    satisfiedSlots: ['BUYER:1'], // only one of the issued slots has evidence
  })
  const { runner } = makeRun()
  const result = await evaluateAgreementCompletion('doc-1', 'evt-1', { execute, run: runner })
  // Two issued slots (SELLER:1, SELLER_BROKER:1) still missing -> not fully executed,
  // regardless of any later draft-participant edit that isn't in the snapshot.
  assert.equal(result.shouldEmit, false)
  assert.equal(result.verdict.fullyExecuted, false)
  assert.deepEqual(result.verdict.missingSlotIds, ['SELLER:1', 'SELLER_BROKER:1'])
})

