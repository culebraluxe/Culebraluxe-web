import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  canonicalizeExecutionParticipants,
  parseIssuedParticipants,
} from '../../lib/agreements/participants'
import { evaluateAgreementCompletion } from '../../lib/agreements/completion'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

// CRM-27 Phase-1 second pass — participant-identity/cardinality closure proofs.

// --- 1. Canonical participant dedup boundary ---------------------------------

test('1. overlapping deal_participant + seeded form participants collapse to ONE slot per actual role/person', () => {
  // The SAME actual Buyer appears twice within the BUYER role: once as a seeded
  // document_form_participant and once as a source deal_participant (same
  // personId b1). After canonicalization there is exactly ONE BUYER slot.
  const slots = canonicalizeExecutionParticipants([
    { role: 'BUYER', personId: 'b1', name: 'Buyer A', email: 'buyer@x.com' }, // seeded form participant
    { role: 'BUYER', personId: 'b1', name: 'Buyer A', email: 'buyer@x.com' }, // source deal participant (overlap)
    { role: 'SELLER', personId: 's1', name: 'Seller', email: 'seller@x.com' },
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: 'broker@x.com' },
  ])
  const buyers = slots.filter((s) => s.role === 'BUYER')
  assert.equal(buyers.length, 1, 'overlapping strong identity dedups to one slot')
  assert.equal(buyers[0].slotId, 'BUYER:1')
  assert.equal(slots.length, 3, 'one required slot per actual role/person identity')
})

test('2. two distinct Buyers remain two required slots', () => {
  const slots = canonicalizeExecutionParticipants([
    { role: 'BUYER', personId: 'b1', name: 'Buyer One', email: 'b1@x.com' },
    { role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: 'b2@x.com' },
  ])
  const buyers = slots.filter((s) => s.role === 'BUYER')
  assert.equal(buyers.length, 2)
  assert.deepEqual(buyers.map((s) => s.slotId).sort(), ['BUYER:1', 'BUYER:2'])
})

test('3. the same participant set in different source ordering yields deterministic slot ids', () => {
  const setA = [
    { role: 'SELLER', personId: 's1', name: 'Seller', email: 'seller@x.com' },
    { role: 'BUYER', personId: 'b1', name: 'Buyer', email: 'buyer@x.com' },
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: 'broker@x.com' },
  ]
  const setB = [
    { role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: 'broker@x.com' },
    { role: 'BUYER', personId: 'b1', name: 'Buyer', email: 'buyer@x.com' },
    { role: 'SELLER', personId: 's1', name: 'Seller', email: 'seller@x.com' },
  ]
  const a = canonicalizeExecutionParticipants(setA)
  const b = canonicalizeExecutionParticipants(setB)
  assert.deepEqual(
    a.map((s) => s.slotId),
    b.map((s) => s.slotId),
    'the same resolved set in any order yields identical slot identities',
  )
  assert.deepEqual(a.map((s) => s.slotId).sort(), ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1'])
})

// --- completion helpers ------------------------------------------------------

const VALID_PARTICIPANTS = [
  { slotId: 'BUYER:1', role: 'BUYER', personId: 'p1', name: 'Buyer One', email: 'buyer1@x.com', required: true, order: 0 },
  { slotId: 'SELLER:1', role: 'SELLER', personId: 'p2', name: 'Seller', email: 'seller@x.com', required: true, order: 1 },
  { slotId: 'SELLER_BROKER:1', role: 'SELLER_BROKER', personId: 'p3', name: 'Broker', email: 'broker@x.com', required: true, order: 2 },
]

function makeExec(opts: { snapshot?: unknown; satisfied?: string[] }) {
  const executor: QueryExecutor = async (strings) => {
    const sql = strings.join('?')
    if (sql.includes('from transaction_document')) {
      return [
        {
          id: 'doc-1',
          template_id: 'PR-PNS',
          issued_version: 1,
          deal_id: 'deal-1',
          document_type: 'agreement',
          source_snapshot: opts.snapshot,
        },
      ]
    }
    if (sql.includes('from signature_request')) {
      return (opts.satisfied ?? []).map((slot) => ({ execution_slot_id: slot }))
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

// --- 4. Strict immutable-snapshot validation (fail closed) --------------------

test('4. missing / malformed / duplicate issuedParticipants cannot automatically complete', async () => {
  const { runner } = makeRun()
  const satisfied = ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1']

  // Missing snapshot (no issuedParticipants key) -> validation_failure.
  const missing = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({ snapshot: {}, satisfied }),
    run: runner,
  })
  assert.equal(missing.outcome, 'validation_failure')
  assert.equal(missing.shouldEmit, false)

  // Null snapshot -> validation_failure.
  const nullSnap = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({ snapshot: null, satisfied }),
    run: runner,
  })
  assert.equal(nullSnap.outcome, 'validation_failure')
  assert.equal(nullSnap.shouldEmit, false)

  // Malformed entry -> validation_failure.
  const malformed = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({ snapshot: { issuedParticipants: [{ role: 'BUYER', name: 'X' }] }, satisfied }),
    run: runner,
  })
  assert.equal(malformed.outcome, 'validation_failure')

  // Duplicate slot ids -> validation_failure.
  const dup = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({
      snapshot: {
        issuedParticipants: [
          VALID_PARTICIPANTS[0],
          { ...VALID_PARTICIPANTS[0], order: 9 },
          VALID_PARTICIPANTS[1],
          VALID_PARTICIPANTS[2],
        ],
      },
      satisfied,
    }),
    run: runner,
  })
  assert.equal(dup.outcome, 'validation_failure')
  assert.equal(dup.shouldEmit, false)
})

test('10. two Buyers require two separate completed slot-bound requests before emission', async () => {
  const twoBuyers = [
    { slotId: 'BUYER:1', role: 'BUYER', personId: 'b1', name: 'Buyer One', email: 'b1@x.com', required: true, order: 0 },
    { slotId: 'BUYER:2', role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: 'b2@x.com', required: true, order: 1 },
    { slotId: 'SELLER:1', role: 'SELLER', personId: 's1', name: 'Seller', email: 's@x.com', required: true, order: 2 },
    { slotId: 'SELLER_BROKER:1', role: 'SELLER_BROKER', personId: 'br1', name: 'Broker', email: 'br@x.com', required: true, order: 3 },
  ]
  const { runner } = makeRun()
  // Only one Buyer completed -> not fully executed.
  const partial = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({ snapshot: { issuedParticipants: twoBuyers }, satisfied: ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1'] }),
    run: runner,
  })
  assert.equal(partial.shouldEmit, false)
  assert.deepEqual(partial.verdict.missingSlotIds, ['BUYER:2'])
  // Both Buyers completed -> fully executed.
  const full = await evaluateAgreementCompletion('doc-1', 'evt', {
    execute: makeExec({ snapshot: { issuedParticipants: twoBuyers }, satisfied: ['BUYER:1', 'BUYER:2', 'SELLER:1', 'SELLER_BROKER:1'] }),
    run: runner,
  })
  assert.equal(full.shouldEmit, true)
})

test('locally composed owner signature is immutable execution-slot evidence', async () => {
  const { runner } = makeRun()
  const result = await evaluateAgreementCompletion('doc-1', 'evt-local-owner', {
    execute: makeExec({
      snapshot: {
        issuedParticipants: VALID_PARTICIPANTS,
        appliedSignatures: [
          {
            role: 'SELLER_BROKER',
            slotId: 'SELLER_BROKER:1',
            signerName: 'Lisa Penfield',
            credentialLine: 'Real Estate Broker License #: C-9931',
            signerAppUserId: 'owner-user',
            assetMediaId: 'protected-media',
            assetChecksumSha256: 'a'.repeat(64),
            appliedAt: '2026-08-26T18:30:00.000Z',
            consentBasis: 'authenticated-owner-issuance',
            dateSemantic: 'issuance-requested-at',
            renderedDate: 'August 26, 2026',
            pageIndex: 3,
            signatureRect: { x: 52, y: 130, width: 302, height: 34 },
            dateRect: { x: 374, y: 130, width: 186, height: 20 },
          },
        ],
      },
      satisfied: ['BUYER:1', 'SELLER:1'],
    }),
    run: runner,
  })
  assert.equal(result.outcome, 'success')
  assert.equal(result.verdict.fullyExecuted, true)
  assert.equal(result.shouldEmit, true)
})

test('parseIssuedParticipants: a role inconsistent with its slot id is rejected', () => {
  const parsed = parseIssuedParticipants([
    { slotId: 'BUYER:1', role: 'SELLER', personId: 'p', name: 'X', email: 'x@x.com', required: true, order: 0 },
  ])
  assert.equal(parsed.ok, false)
})


// --- 5/6/7/8/9. Canonical send-boundary enforcement ---------------------------

import { sendSignatureRequest } from '../../db/signature-request'

type SendFakeState = {
  docSnapshot?: unknown
  active?: { id: string; execution_slot_id: string | null }
  conflictOnInsert?: boolean
  recipientRows?: Array<{
    signatureRequestId: string
    executionRole: string | null
    executionSlotId: string | null
    email: string
    order: number
  }>
}

function makeSendRun(state: SendFakeState) {
  const claimed = new Set<string>()
  const runner: TxRunner = (cb) =>
    cb((async (strings, ...params: any[]) => {
      const sql = strings.join('?')
      if (sql.includes('insert into workflow_command_receipt') && sql.includes('on conflict')) {
        if (claimed.has(String(params[0]))) return []
        claimed.add(String(params[0]))
        return Promise.resolve([{ command_id: params[0] }])
      }
      if (sql.includes('update workflow_command_receipt set outcome')) return Promise.resolve([])
      if (sql.includes('select command_id, outcome')) return Promise.resolve([])
      if (sql.includes('from transaction_document') && sql.includes('source_snapshot')) {
        return state.docSnapshot === undefined
          ? Promise.resolve([])
          : Promise.resolve([{ id: 'doc-1', source_snapshot: state.docSnapshot }])
      }
      if (sql.includes('from transaction_document')) return Promise.resolve([{ id: 'doc-1' }])
      if (sql.includes('insert into signature_request')) {
        if (state.conflictOnInsert) return Promise.resolve([])
        return Promise.resolve([
          {
            id: 'sig-1',
            transaction_document_id: 'doc-1',
            status: 'requested',
            message: params[1] ?? null,
            execution_role: params[3] ?? null,
            execution_slot_id: params[4] ?? null,
            created_by_user_id: params[2] ?? null,
            created_at: '2026-08-24T12:00:00.000Z',
            updated_at: '2026-08-24T12:00:00.000Z',
          },
        ])
      }
      if (sql.includes('insert into signature_envelope_recipient')) {
        state.recipientRows?.push({
          signatureRequestId: String(params[0]),
          executionRole: params[1] ? String(params[1]) : null,
          executionSlotId: params[2] ? String(params[2]) : null,
          email: String(params[4]),
          order: Number(params[5]),
        })
        return Promise.resolve([])
      }
      if (sql.includes('from signature_request') && sql.includes('status in')) {
        if (!state.active) return Promise.resolve([])
        return Promise.resolve([
          {
            id: state.active.id,
            transaction_document_id: 'doc-1',
            status: 'requested',
            message: null,
            execution_role: null,
            execution_slot_id: state.active.execution_slot_id,
            created_by_user_id: null,
            created_at: '2026-08-24T12:00:00.000Z',
            updated_at: '2026-08-24T12:00:00.000Z',
          },
        ])
      }
      return Promise.resolve([])
    }) as QueryExecutor)
  return runner
}

const VALID_SNAPSHOT = { issuedParticipants: VALID_PARTICIPANTS }
const SELLER_ONLY_SNAPSHOT = { issuedParticipants: [VALID_PARTICIPANTS[1]] }
const TWO_BUYER_SNAPSHOT = {
  issuedParticipants: [
    ...VALID_PARTICIPANTS,
    { slotId: 'BUYER:2', role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: 'buyer2@x.com', required: true, order: 3 },
  ],
}

test('5. a nonexistent slot is rejected at signature send', async () => {
  const result = await sendSignatureRequest(
    {
      commandId: 'c5',
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:99',
      slotRecipientEmail: 'buyer1@x.com',
    },
    makeSendRun({ docSnapshot: VALID_SNAPSHOT }),
  )
  assert.equal(result.outcome, 'validation_failure')
  assert.match(result.message ?? '', /does not exist/)
})

test('6. a slot belonging to document A cannot be attached to document B', async () => {
  // Document B's snapshot has NO BUYER:1 (only a SELLER slot). Sending BUYER:1
  // against document B must be rejected even though "BUYER:1" is a valid literal
  // slot label in another document.
  const result = await sendSignatureRequest(
    {
      commandId: 'c6',
      transactionDocumentId: 'doc-B',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'buyer1@x.com',
    },
    makeSendRun({ docSnapshot: SELLER_ONLY_SNAPSHOT }),
  )
  assert.equal(result.outcome, 'validation_failure')
  assert.match(result.message ?? '', /does not exist in document doc-B/)
})

test('7. a recipient that does not match the immutable slot is rejected', async () => {
  const result = await sendSignatureRequest(
    {
      commandId: 'c7',
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'someone.else@x.com',
    },
    makeSendRun({ docSnapshot: VALID_SNAPSHOT }),
  )
  assert.equal(result.outcome, 'validation_failure')
  assert.match(result.message ?? '', /does not match the immutable execution slot/)
})

test('8. a slot-bound send persists the validated executionRole and executionSlotId', async () => {
  const result = await sendSignatureRequest(
    {
      commandId: 'c8',
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'buyer1@x.com',
    },
    makeSendRun({ docSnapshot: VALID_SNAPSHOT }),
  )
  assert.equal(result.outcome, 'success')
  const req = (result.value as { signatureRequest?: { executionRole: string | null; executionSlotId: string | null } }).signatureRequest
  assert.equal(req?.executionRole, 'BUYER')
  assert.equal(req?.executionSlotId, 'BUYER:1')
})

test('one canonical envelope persists four independently slot-bound recipients', async () => {
  const extra = [
    { slotId: 'BUYER:2', role: 'BUYER', personId: 'b2', name: 'Buyer Two', email: 'buyer2@x.com', required: true, order: 3 },
    { slotId: 'SELLER:2', role: 'SELLER', personId: 's2', name: 'Seller Two', email: 'seller2@x.com', required: true, order: 4 },
  ]
  const snapshot = { issuedParticipants: [...VALID_PARTICIPANTS, ...extra] }
  const rows: NonNullable<SendFakeState['recipientRows']> = []
  const recipients = [
    { role: 'signer' as const, name: 'Buyer One', email: 'buyer1@x.com', order: 1, executionRole: 'BUYER', executionSlotId: 'BUYER:1' },
    { role: 'signer' as const, name: 'Buyer Two', email: 'buyer2@x.com', order: 2, executionRole: 'BUYER', executionSlotId: 'BUYER:2' },
    { role: 'signer' as const, name: 'Seller', email: 'seller@x.com', order: 3, executionRole: 'SELLER', executionSlotId: 'SELLER:1' },
    { role: 'signer' as const, name: 'Seller Two', email: 'seller2@x.com', order: 4, executionRole: 'SELLER', executionSlotId: 'SELLER:2' },
  ]
  const result = await sendSignatureRequest(
    {
      commandId: 'c-envelope-4',
      transactionDocumentId: 'doc-1',
      recipients,
    },
    makeSendRun({ docSnapshot: snapshot, recipientRows: rows }),
  )
  assert.equal(result.outcome, 'success')
  assert.equal(rows.length, 4)
  assert.deepEqual(rows.map((row) => row.executionSlotId), [
    'BUYER:1', 'BUYER:2', 'SELLER:1', 'SELLER:2',
  ])
  assert.deepEqual(rows.map((row) => row.order), [1, 2, 3, 4])
})


test('9. duplicate send for the same active slot returns the existing request; a different active slot returns conflict', async () => {
  // Same slot: the active request is BUYER:1; resending BUYER:1 returns it.
  const same = await sendSignatureRequest(
    {
      commandId: 'c9a',
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'buyer1@x.com',
    },
    makeSendRun({
      docSnapshot: VALID_SNAPSHOT,
      conflictOnInsert: true,
      active: { id: 'sig-active', execution_slot_id: 'BUYER:1' },
    }),
  )
  assert.equal(same.outcome, 'success')
  assert.equal((same.value as { signatureRequest?: { id: string } }).signatureRequest?.id, 'sig-active')

  // Different slot: active is BUYER:1; sending BUYER:2 must be a truthful conflict.
  const different = await sendSignatureRequest(
    {
      commandId: 'c9b',
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:2',
      slotRecipientEmail: 'buyer2@x.com',
    },
    makeSendRun({
      docSnapshot: TWO_BUYER_SNAPSHOT,
      conflictOnInsert: true,
      active: { id: 'sig-active', execution_slot_id: 'BUYER:1' },
    }),
  )
  assert.equal(different.outcome, 'conflict')
})

// --- Correction 2: bind the ACTUAL provider recipient to the slot -------------

import { SendSignatureRequestCommand } from '../../lib/commands/signature/signature-commands'
import { SIGNATURE_REQUEST_SEND } from '../../lib/commands/command-types'
import type { CommandEnvelope } from '../../lib/workflow/contracts'
import type { CommandExecutionContext } from '../../lib/commands/contracts'

const sendHandler = new SendSignatureRequestCommand()

function sendEnvelope(input: Record<string, unknown>): CommandEnvelope {
  return {
    commandId: 'c-send',
    commandType: SIGNATURE_REQUEST_SEND,
    actorAppUserId: null,
    aggregateType: 'signature_request',
    aggregateId: null,
    correlationId: null,
    causationId: null,
    requestedAt: '2026-08-24T12:00:00.000Z',
    input,
  } as CommandEnvelope
}

// Rejection paths return before touching ctx, so a minimal ctx is enough.
const inertCtx = {} as unknown as CommandExecutionContext

test('correction 2: an actual recipient mismatch is rejected even when slotRecipientEmail is correct', async () => {
  const result = await sendHandler.handle(
    sendEnvelope({
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'buyer1@x.com',
      recipients: [{ role: 'signer', name: 'Wrong Person', email: 'someone.else@x.com', order: 1 }],
    }),
    inertCtx,
  )
  assert.equal(result.outcome, 'validation_failure')
  assert.match(result.message ?? '', /must match the immutable execution slot/)
})

test('correction 2: a slot-bound send missing slotRecipientEmail is rejected', async () => {
  const result = await sendHandler.handle(
    sendEnvelope({
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      recipients: [{ role: 'signer', name: 'Buyer One', email: 'buyer1@x.com', order: 1 }],
    }),
    inertCtx,
  )
  assert.equal(result.outcome, 'validation_failure')
  assert.match(result.message ?? '', /requires slotRecipientEmail/)
})

test('correction 2: a slot-bound send with the correct actual recipient succeeds', async () => {
  const ctx = {
    run: makeSendRun({ docSnapshot: VALID_SNAPSHOT }),
  } as unknown as CommandExecutionContext
  const result = await sendHandler.handle(
    sendEnvelope({
      transactionDocumentId: 'doc-1',
      executionRole: 'BUYER',
      executionSlotId: 'BUYER:1',
      slotRecipientEmail: 'buyer1@x.com',
      recipients: [{ role: 'signer', name: 'Buyer One', email: 'buyer1@x.com', order: 1 }],
    }),
    ctx,
  )
  assert.equal(result.outcome, 'success')
  const req = (result.value as { signatureRequest?: { executionRole: string | null; executionSlotId: string | null } }).signatureRequest
  assert.equal(req?.executionRole, 'BUYER')
  assert.equal(req?.executionSlotId, 'BUYER:1')
})

// --- Correction 3: active-slot decision (production path) --------------------

import { decideActiveSlotSend } from '../../lib/agreements/participants'

test('correction 3: active BUYER:1 + selected BUYER:1 is a same-slot replay (existing)', () => {
  assert.deepEqual(decideActiveSlotSend('BUYER:1', 'BUYER:1'), { kind: 'existing' })
})

test('correction 3: active BUYER:1 + selected BUYER:2 is a truthful conflict', () => {
  assert.deepEqual(decideActiveSlotSend('BUYER:1', 'BUYER:2'), {
    kind: 'conflict',
    activeSlotId: 'BUYER:1',
    requestedSlotId: 'BUYER:2',
  })
})

test('correction 3: non-slot-bound (generic) sends are not gated by an active slot', () => {
  assert.deepEqual(decideActiveSlotSend('BUYER:1', null), { kind: 'none' })
  assert.deepEqual(decideActiveSlotSend(null, 'BUYER:1'), { kind: 'none' })
})
