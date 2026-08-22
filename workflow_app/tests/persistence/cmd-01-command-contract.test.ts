// ---------------------------------------------------------------------------
// CMD-01 — Canonical Business Command Envelope + Execution Contract: SCOPED
// focused tests (5), DB-backed against the DEV control plane.
//
// Dangerous semantics only (per story test policy):
//   1. ATOMIC COMMIT — mutation + receipt + outbox event land in ONE
//      transaction (deal.set_stage_under_contract; chain metadata survives on
//      receipt AND event).
//   2. REPLAY — the same commandId never duplicates a business effect, receipt
//      or outbox fact; the dispatcher fast-path reports replayed=true.
//   3. DETERMINISTIC REJECTION — validation/precondition failures return an
//      explicit category, a non-empty message and NO outbox fact; a claimed
//      failure is persisted as a failure receipt with its chain.
//   4. TECHNICAL FAILURE — a thrown error inside the transaction rolls back
//      mutation, receipt and outbox as one unit.
//   5. SECOND PROOF COMMAND — offer.accept runs through the SAME generic
//      dispatcher contract (mutation + receipt + OFFER_ACCEPTED outbox).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import { interactiveSql } from '../../../lib/neon-interactive'
import { neonTx } from '../../../db/tx'
import type { QueryExecutor } from '../../../db/query-executor'
import type { TxRunner } from '../../../db/tx'
import type { OutboxEventRepository } from '../../../lib/events/outbox-contracts'
import { PostgresCommandReceiptRepository } from '../../../db/command-receipt-repository'
import { CommandDispatcherImpl } from '../../../lib/commands/dispatcher'
import { createCommandRegistry } from '../../../lib/commands/register'
import { PostgresOutboxEventRepository } from '../../../lib/mq/outbox-repository'
import type { CommandEnvelope } from '../../../lib/workflow/contracts'
import {
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_STAGE_UNDER_CONTRACT,
  OFFER_ACCEPT,
} from '../../../lib/commands/command-types'

type Row = Record<string, any>

const executor = interactiveSql as unknown as QueryExecutor
const dbExecutor = () => Promise.resolve(executor)

const receipts = new PostgresCommandReceiptRepository()
const outbox = new PostgresOutboxEventRepository(dbExecutor)
const registry = createCommandRegistry()

function makeDispatcher(
  run: TxRunner = neonTx,
  eventSink?: OutboxEventRepository | null,
): CommandDispatcherImpl {
  return new CommandDispatcherImpl({
    registry,
    receipts,
    run,
    eventSink: eventSink === undefined ? outbox : eventSink,
  })
}

// ---------------------------------------------------------------------------
// Fixtures (DEV DB)
// ---------------------------------------------------------------------------

const created = {
  userIds: [] as string[],
  personIds: [] as string[],
  propertyIds: [] as string[],
  dealIds: [] as string[],
  offerIds: [] as string[],
}

async function createFixture(): Promise<{
  userId: string
  dealId: string
  submittedOfferId: string
  withdrawnOfferId: string
}> {
  const userRows = await interactiveSql`
    insert into app_user (display_name) values ('cmd01-' || ${randomUUID()}) returning id
  `
  const userId = (userRows[0] as Row).id as string
  created.userIds.push(userId)

  const personRows = await interactiveSql`
    insert into person (display_name, role, status)
    values ('cmd01-person-' || ${randomUUID()}, 'buyer', 'active') returning id
  `
  const personId = (personRows[0] as Row).id as string
  created.personIds.push(personId)

  const propertyRows = await interactiveSql`
    insert into property (name, location)
    values ('cmd01-property-' || ${randomUUID()}, 'Culebra') returning id
  `
  const propertyId = (propertyRows[0] as Row).id as string
  created.propertyIds.push(propertyId)

  const dealRows = await interactiveSql`
    insert into deal (property_id, client_person_id, stage)
    values (${propertyId}, ${personId}, 'offer') returning id
  `
  const dealId = (dealRows[0] as Row).id as string
  created.dealIds.push(dealId)

  const submittedRows = await interactiveSql`
    insert into offer (deal_id, person_id, amount, status)
    values (${dealId}, ${personId}, 1, 'submitted') returning id
  `
  const submittedOfferId = (submittedRows[0] as Row).id as string
  created.offerIds.push(submittedOfferId)

  const withdrawnRows = await interactiveSql`
    insert into offer (deal_id, person_id, amount, status)
    values (${dealId}, ${personId}, 1, 'withdrawn') returning id
  `
  const withdrawnOfferId = (withdrawnRows[0] as Row).id as string
  created.offerIds.push(withdrawnOfferId)

  return { userId, dealId, submittedOfferId, withdrawnOfferId }
}

async function cleanup(): Promise<void> {
  await interactiveSql`delete from mq_proof_effect`
  await interactiveSql`delete from mq_delivery`
  await interactiveSql`delete from outbox_message where correlation_id like 'cmd01-corr-%'`
  await interactiveSql`delete from workflow_command_receipt where command_id like 'cmd01-%'`
  for (const id of created.offerIds) await interactiveSql`delete from offer where id = ${id}`
  for (const id of created.dealIds) await interactiveSql`delete from deal where id = ${id}`
  for (const id of created.propertyIds) await interactiveSql`delete from property where id = ${id}`
  for (const id of created.personIds) await interactiveSql`delete from person where id = ${id}`
  for (const id of created.userIds) await interactiveSql`delete from app_user where id = ${id}`
  created.userIds = []
  created.personIds = []
  created.propertyIds = []
  created.dealIds = []
  created.offerIds = []
}

// ---------------------------------------------------------------------------
// Envelope helpers
// ---------------------------------------------------------------------------

function stageEnvelope(
  dealId: string,
  overrides: Partial<CommandEnvelope> = {},
): CommandEnvelope {
  return {
    commandId: 'cmd01-stage-' + randomUUID(),
    commandType: DEAL_SET_STAGE_UNDER_CONTRACT,
    actorAppUserId: null,
    aggregateType: 'deal',
    aggregateId: dealId,
    correlationId: 'cmd01-corr-' + randomUUID(),
    causationId: 'cmd01-cause-' + randomUUID(),
    requestedAt: new Date().toISOString(),
    input: { dealId },
    ...overrides,
  }
}

function stageClosedEnvelope(dealId: string): CommandEnvelope {
  return {
    commandId: 'cmd01-stageclosed-' + randomUUID(),
    commandType: DEAL_SET_STAGE_CLOSED,
    actorAppUserId: null,
    aggregateType: 'deal',
    aggregateId: dealId,
    correlationId: 'cmd01-corr-' + randomUUID(),
    causationId: 'cmd01-cause-' + randomUUID(),
    requestedAt: new Date().toISOString(),
    input: { dealId },
  }
}

function acceptEnvelope(
  dealId: string,
  offerId: string,
  overrides: Partial<CommandEnvelope> = {},
): CommandEnvelope {
  return {
    commandId: 'cmd01-accept-' + randomUUID(),
    commandType: OFFER_ACCEPT,
    actorAppUserId: null,
    aggregateType: 'deal',
    aggregateId: dealId,
    correlationId: 'cmd01-corr-' + randomUUID(),
    causationId: 'cmd01-cause-' + randomUUID(),
    requestedAt: new Date().toISOString(),
    input: { dealId, offerId },
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// 1. ATOMIC COMMIT — mutation + receipt + outbox event in ONE transaction
// ---------------------------------------------------------------------------

test('CMD-01 atomic commit: mutation + receipt + outbox event share one transaction', async () => {
  const fx = await createFixture()
  try {
    const envelope = stageEnvelope(fx.dealId, {
      commandId: 'cmd01-stage-atomic-' + randomUUID(),
      actorAppUserId: fx.userId,
    })
    const result = await makeDispatcher().execute(envelope)

    // Explicit result contract: outcome + stamped command type + no replay.
    assert.equal(result.outcome, 'success')
    assert.equal(result.commandType, DEAL_SET_STAGE_UNDER_CONTRACT)
    assert.equal(result.replayed, false)

    // Mutation committed.
    const dealRows = await interactiveSql`
      select stage from deal where id = ${fx.dealId}
    `
    assert.equal((dealRows[0] as Row).stage, 'under_contract')

    // Receipt committed with outcome + the envelope chain facts.
    const receiptRows = await interactiveSql`
      select outcome, command_type, correlation_id, causation_id, actor_app_user_id
      from workflow_command_receipt where command_id = ${envelope.commandId}
    `
    assert.equal(receiptRows.length, 1)
    const receipt = receiptRows[0] as Row
    assert.equal(receipt.outcome, 'success')
    assert.equal(receipt.command_type, DEAL_SET_STAGE_UNDER_CONTRACT)
    assert.equal(receipt.correlation_id, envelope.correlationId)
    assert.equal(receipt.causation_id, envelope.causationId)
    assert.equal(receipt.actor_app_user_id, fx.userId)

    // Outbox fact committed in the SAME transaction with the same chain
    // (causation defaults to the commandId per the correlation contract).
    const outRows = await interactiveSql`
      select event_type, correlation_id, causation_id, actor_app_user_id
      from outbox_message where correlation_id = ${envelope.correlationId}
    `
    assert.equal(outRows.length, 1)
    const fact = outRows[0] as Row
    assert.equal(fact.event_type, 'DEAL_STAGE_CHANGED')
    assert.equal(fact.correlation_id, envelope.correlationId)
    assert.equal(fact.causation_id, envelope.commandId)
    assert.equal(fact.actor_app_user_id, fx.userId)
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// 2. REPLAY — same commandId never duplicates effect, receipt or outbox fact
// ---------------------------------------------------------------------------

test('CMD-01 replay: same commandId replays without duplicate effect or outbox fact', async () => {
  const fx = await createFixture()
  try {
    const commandId = 'cmd01-stage-replay-' + randomUUID()
    const correlationId = 'cmd01-corr-replay-' + randomUUID()
    const envelope = stageEnvelope(fx.dealId, { commandId, correlationId })
    const dispatcher = makeDispatcher()

    const first = await dispatcher.execute(envelope)
    assert.equal(first.outcome, 'success')
    assert.equal(first.replayed, false)

    // A sentinel mutation proves a duplicate run (if any) would be visible.
    await interactiveSql`update deal set financing_type = 'cash' where id = ${fx.dealId}`

    const second = await dispatcher.execute(
      stageEnvelope(fx.dealId, { commandId, correlationId }),
    )
    assert.equal(second.outcome, 'success')
    assert.equal(second.replayed, true)
    assert.equal(second.commandType, DEAL_SET_STAGE_UNDER_CONTRACT)
    assert.equal(second.emittedEvents.length, 0)

    const receiptCount = (
      await interactiveSql`
        select count(*)::int as n from workflow_command_receipt where command_id = ${commandId}
      `
    )[0] as Row
    assert.equal(receiptCount.n, 1)

    const outCount = (
      await interactiveSql`
        select count(*)::int as n from outbox_message where correlation_id = ${correlationId}
      `
    )[0] as Row
    assert.equal(outCount.n, 1)

    // CMD-01: an idempotencyKey that is not the commandId is a contract
    // violation — a validation rejection, never a duplicate attempt.
    const bad = await dispatcher.execute(
      stageEnvelope(fx.dealId, { idempotencyKey: 'not-the-command-id' }),
    )
    assert.equal(bad.outcome, 'validation_failure')
    assert.equal(bad.error?.category, 'validation')
    assert.match(bad.error?.message ?? '', /idempotencyKey/)
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// 3. DETERMINISTIC REJECTION — explicit category, non-empty message, no fact
// ---------------------------------------------------------------------------

test('CMD-01 deterministic rejection: validation category + non-empty message + NO outbox fact', async () => {
  const fx = await createFixture()
  try {
    const dispatcher = makeDispatcher()

    // 3a — rejected via compare-and-set (expected 'under_contract', was
    // 'offer'): a deterministic retryable conflict with the explicit category
    // + a non-empty message; nothing is emitted.
    const stageClosedEnvelopeInstance = stageClosedEnvelope(fx.dealId)
    const badStage = await dispatcher.execute(stageClosedEnvelopeInstance)
    assert.equal(badStage.outcome, 'conflict')
    assert.equal(badStage.error?.category, 'validation')
    assert.equal(badStage.error?.retryable, true)
    assert.ok((badStage.error?.message ?? '').length > 0)
    assert.equal(badStage.replayed, false)

    // 3b — claimed then rejected (precondition_failure): the FAILURE receipt
    // is persisted WITH its chain; still no outbox fact.
    const badAcceptEnvelope = acceptEnvelope(fx.dealId, fx.withdrawnOfferId)
    const badAccept = await dispatcher.execute(badAcceptEnvelope)
    assert.equal(badAccept.outcome, 'precondition_failure')
    assert.equal(badAccept.error?.category, 'validation')
    assert.ok((badAccept.error?.message ?? '').length > 0)

    const receiptRows = await interactiveSql`
      select outcome, command_type, correlation_id, causation_id
      from workflow_command_receipt where command_id = ${badAcceptEnvelope.commandId}
    `
    assert.equal(receiptRows.length, 1)
    assert.equal((receiptRows[0] as Row).outcome, 'precondition_failure')
    assert.equal((receiptRows[0] as Row).command_type, OFFER_ACCEPT)
    assert.equal(
      (receiptRows[0] as Row).correlation_id,
      badAcceptEnvelope.correlationId,
    )

    for (const corr of [
      stageClosedEnvelopeInstance.correlationId,
      badAcceptEnvelope.correlationId,
    ]) {
      const outCount = (
        await interactiveSql`
          select count(*)::int as n from outbox_message where correlation_id = ${corr}
        `
      )[0] as Row
      assert.equal(outCount.n, 0, `no outbox fact for ${corr}`)
    }
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// 4. TECHNICAL FAILURE — a thrown error rolls back mutation + receipt + outbox
// ---------------------------------------------------------------------------

test('CMD-01 technical failure: throw inside the transaction leaves NO partial state', async () => {
  const fx = await createFixture()
  try {
    const throwingSink = {
      append: async (): Promise<void> => {
        throw new Error('simulated outbox technical failure')
      },
    } as unknown as OutboxEventRepository

    const envelope = acceptEnvelope(fx.dealId, fx.submittedOfferId)
    await assert.rejects(
      () => makeDispatcher(neonTx, throwingSink).execute(envelope),
      /simulated outbox technical failure/,
    )

    // Mutation rolled back.
    const offerRows = await interactiveSql`
      select status from offer where id = ${fx.submittedOfferId}
    `
    assert.equal((offerRows[0] as Row).status, 'submitted')

    // Receipt rolled back (the claim row disappeared with the transaction).
    const receiptCount = (
      await interactiveSql`
        select count(*)::int as n from workflow_command_receipt where command_id = ${envelope.commandId}
      `
    )[0] as Row
    assert.equal(receiptCount.n, 0)

    // No deliverable outbox fact.
    const outCount = (
      await interactiveSql`
        select count(*)::int as n from outbox_message where correlation_id = ${envelope.correlationId}
      `
    )[0] as Row
    assert.equal(outCount.n, 0)
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// 5. SECOND PROOF COMMAND — offer.accept through the same generic dispatcher
// ---------------------------------------------------------------------------

test('CMD-01 second proof command: offer.accept commits mutation + receipt + OFFER_ACCEPTED outbox', async () => {
  const fx = await createFixture()
  try {
    const envelope = acceptEnvelope(fx.dealId, fx.submittedOfferId, {
      commandId: 'cmd01-accept-ok-' + randomUUID(),
      actorAppUserId: fx.userId,
    })
    const result = await makeDispatcher().execute(envelope)

    assert.equal(result.outcome, 'success')
    assert.equal(result.commandType, OFFER_ACCEPT)
    assert.equal(result.replayed, false)

    const offerRows = await interactiveSql`
      select status from offer where id = ${fx.submittedOfferId}
    `
    assert.equal((offerRows[0] as Row).status, 'accepted')

    const receiptRows = await interactiveSql`
      select outcome, command_type, correlation_id, causation_id, actor_app_user_id
      from workflow_command_receipt where command_id = ${envelope.commandId}
    `
    assert.equal(receiptRows.length, 1)
    assert.equal((receiptRows[0] as Row).outcome, 'success')
    assert.equal((receiptRows[0] as Row).command_type, OFFER_ACCEPT)
    assert.equal((receiptRows[0] as Row).correlation_id, envelope.correlationId)
    assert.equal((receiptRows[0] as Row).causation_id, envelope.causationId)
    assert.equal((receiptRows[0] as Row).actor_app_user_id, fx.userId)

    const outRows = await interactiveSql`
      select event_type, correlation_id, causation_id
      from outbox_message where correlation_id = ${envelope.correlationId}
    `
    assert.equal(outRows.length, 1)
    assert.equal((outRows[0] as Row).event_type, 'OFFER_ACCEPTED')
    assert.equal((outRows[0] as Row).correlation_id, envelope.correlationId)
    assert.equal((outRows[0] as Row).causation_id, envelope.commandId)
  } finally {
    await cleanup()
  }
})




