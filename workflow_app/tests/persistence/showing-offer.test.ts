// ---------------------------------------------------------------------------
// CRM-11 — Showings + Offers (targeted).
//
// SCOPED tests only: the changed seams (showing completion interaction
// emission, showing source-interaction linkage, offer lineage, the offer.accept
// command seam) plus directly adjacent behavior (portal offer writes, no
// deal-stage auto-advance). No full harness, no unrelated regression.
//
// Pure (no DB): offer.accept command seam — the thin handler is registered in
// the canonical command registry and dispatches the canonical service with
// envelope translation, idempotent replay, the one-accepted/primary-offer-per-
// deal invariant and no deal-stage auto-advance (in-memory transaction fake,
// the same pattern acceptance.test.ts uses).
//
// Persistence (DEV Neon branch via db/client): the canonical showing/offer
// write surface against the real schema — a completed showing emits exactly
// one canonical interaction idempotently keyed by showing.id; requested /
// scheduled / cancelled emit none; request_source_interaction_id persists when
// provided; counters link via parent_offer_id; withdraw/reject transition
// submitted offers only and never touch deal.stage. Fixtures are tunit-
// prefixed and removed in after().
// ---------------------------------------------------------------------------

import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import { sql } from '../../../db/client'
import {
  cancelShowing,
  completeShowing,
  createShowing,
  rejectOffer,
  scheduleShowing,
  submitOffer,
  withdrawOffer,
} from '../../../db/portal-writes'
import { createCommandRegistry } from '../../../lib/commands/register'
import { OFFER_ACCEPT } from '../../../lib/commands/command-types'
import { AcceptOfferCommand } from '../../../lib/commands/offer/accept-offer'
import type { CommandEnvelope } from '../../../lib/workflow/contracts'
import type { TxRunner } from '../../../db/tx'
import type { QueryExecutor } from '../../../db/query-executor'

type Row = Record<string, any>

// ---------------------------------------------------------------------------
// Pure — offer.accept command seam (envelope translation + invariants)
// ---------------------------------------------------------------------------

/** In-memory transaction mirroring the claim-first receipt + offer SQL. */
class FakeOfferDomain {
  deals: Row[] = []
  offers: Row[] = []
  receipts: Row[] = []

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
    )
    const p = params as any[]

    // Claim: INSERT ... ON CONFLICT (command_id) DO NOTHING RETURNING command_id
    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      const exists = this.receipts.some((r) => r.command_id === p[0])
      if (exists) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    // Finalize: UPDATE receipt (AUTH-05: actor is the 4th SET param)
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[4])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
        r.actor_app_user_id = p[3] ?? null
      }
      return Promise.resolve([])
    }
    // Read final receipt
    if (t.includes('select command_id, outcome, aggregate_id, message') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }] : [])
    }

    if (t.includes('select id from deal where id =') && t.includes('for update')) {
      const row = this.deals.find((d) => d.id === p[0])
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    if (t.includes('select id, deal_id, status from offer where id =') && t.includes('for update')) {
      const row = this.offers.find((o) => o.id === p[0])
      return Promise.resolve(row ? [{ id: row.id, deal_id: row.deal_id, status: row.status }] : [])
    }
    if (t.includes("select id from offer where deal_id =") && t.includes("status = 'accepted'")) {
      const rows = this.offers.filter((o) => o.deal_id === p[0] && o.status === 'accepted')
      return Promise.resolve(rows.map((r) => ({ id: r.id })))
    }
    if (t.includes('update offer set status =') && t.includes("'accepted'")) {
      const row = this.offers.find((o) => o.id === p[0] && o.status === 'submitted')
      if (row) row.status = 'accepted'
      return Promise.resolve([])
    }

    throw new Error(`FAKE_OFFER_DOMAIN_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

function acceptEnvelope(
  commandId: string,
  dealId: string,
  offerId: string,
): CommandEnvelope {
  return {
    commandId,
    commandType: OFFER_ACCEPT,
    actorAppUserId: null,
    aggregateType: 'deal',
    aggregateId: dealId,
    correlationId: null,
    causationId: null,
    requestedAt: '2026-09-01T00:00:00.000Z',
    input: { offerId },
  }
}

test('CRM-11: offer.accept is registered in the canonical command registry', () => {
  const registry = createCommandRegistry()
  const handler = registry.resolve(OFFER_ACCEPT)
  assert.ok(
    handler instanceof AcceptOfferCommand,
    'offer.accept resolves to the thin AcceptOfferCommand handler',
  )
})

test('CRM-11: offer.accept seam accepts a submitted offer, replays idempotently, and never auto-advances deal.stage', async () => {
  const f = new FakeOfferDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push({ id: 'offer-1', deal_id: 'deal-1', status: 'submitted' })

  const handler = createCommandRegistry().resolve(OFFER_ACCEPT) as AcceptOfferCommand
  const envelope = acceptEnvelope('cmd-accept-1', 'deal-1', 'offer-1')

  const first = await handler.handle(envelope, { run: f.runner } as any)
  assert.equal(first.outcome, 'success')
  assert.equal(first.aggregateId, 'offer-1')
  assert.equal(f.offers.find((o) => o.id === 'offer-1')!.status, 'accepted')
  assert.equal(f.deals[0].stage, 'offer', 'accept does not auto-advance the deal stage')

  // Same commandId replays the winner result without re-accepting.
  const replay = await handler.handle(envelope, { run: f.runner } as any)
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('CRM-11: offer.accept seam enforces one accepted/primary offer per deal', async () => {
  const f = new FakeOfferDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push(
    { id: 'offer-1', deal_id: 'deal-1', status: 'accepted' },
    { id: 'offer-2', deal_id: 'deal-1', status: 'submitted' },
  )

  const handler = createCommandRegistry().resolve(OFFER_ACCEPT) as AcceptOfferCommand
  const res = await handler.handle(
    acceptEnvelope('cmd-accept-2', 'deal-1', 'offer-2'),
    { run: f.runner } as any,
  )
  assert.equal(res.outcome, 'conflict')
  assert.equal(f.offers.find((o) => o.id === 'offer-2')!.status, 'submitted', 'competing offer preserved')
})

// ---------------------------------------------------------------------------
// Persistence — showing/offer write surface on the DEV Neon branch
// ---------------------------------------------------------------------------

const fixture = {
  personIds: [] as string[],
  propertyIds: [] as string[],
  dealIds: [] as string[],
  showingIds: [] as string[],
  offerIds: [] as string[],
}

async function createFixturePerson(): Promise<string> {
  const rows = await sql`
    insert into person (display_name, role, status)
    values ('tunit-crm11-' || ${randomUUID()}, 'buyer', 'active')
    returning id
  `
  const id = (rows[0] as { id: string }).id
  fixture.personIds.push(id)
  return id
}

async function createFixtureProperty(): Promise<string> {
  const rows = await sql`
    insert into property (name, location)
    values ('tunit-crm11-property-' || ${randomUUID()}, 'Culebra')
    returning id
  `
  const id = (rows[0] as { id: string }).id
  fixture.propertyIds.push(id)
  return id
}

async function createFixtureDeal(personId: string, propertyId: string): Promise<string> {
  const rows = await sql`
    insert into deal (property_id, client_person_id, stage)
    values (${propertyId}, ${personId}, 'offer')
    returning id
  `
  const id = (rows[0] as { id: string }).id
  fixture.dealIds.push(id)
  return id
}

async function createFixtureSourceInteraction(personId: string): Promise<string> {
  const rows = await sql`
    insert into interaction (
      person_id, channel, event_type, occurred_at, title, source_system, source_external_id
    ) values (
      ${personId}, 'website', 'private_viewing_requested', now(),
      'tunit viewing request', 'tunit:website', ${randomUUID()}
    )
    returning id
  `
  return (rows[0] as { id: string }).id
}

async function showingInteractions(showingId: string): Promise<Row[]> {
  return (await sql`
    select id, person_id, property_id, deal_id, channel, event_type,
      occurred_at, title, source_system, source_external_id
    from interaction
    where source_system = 'showing' and source_external_id = ${showingId}
    order by created_at, id
  `) as Row[]
}

after(async () => {
  await sql`delete from offer where id = any(${fixture.offerIds}::uuid[])`
  await sql`delete from showing where id = any(${fixture.showingIds}::uuid[])`
  await sql`delete from interaction where person_id = any(${fixture.personIds}::uuid[])`
  await sql`delete from deal where id = any(${fixture.dealIds}::uuid[])`
  await sql`delete from property where id = any(${fixture.propertyIds}::uuid[])`
  await sql`delete from person where id = any(${fixture.personIds}::uuid[])`
})

test('CRM-11: a completed showing emits exactly one canonical showing interaction, idempotent by showing.id (real Postgres)', async () => {
  const personId = await createFixturePerson()
  const propertyId = await createFixtureProperty()
  const dealId = await createFixtureDeal(personId, propertyId)

  const created = await createShowing({ personId, propertyId, dealId })
  fixture.showingIds.push(created.id)
  assert.equal(created.status, 'requested')

  const scheduled = await scheduleShowing(created.id, '2026-09-01T15:00:00.000Z')
  assert.equal(scheduled.status, 'scheduled')

  const completed = await completeShowing(created.id, '2026-09-01T16:00:00.000Z')
  assert.equal(completed.status, 'completed')

  const interactions = await showingInteractions(created.id)
  assert.equal(interactions.length, 1, 'exactly one canonical interaction on completion')
  const i = interactions[0]
  assert.equal(i.channel, 'showing')
  assert.equal(i.event_type, 'showing_completed')
  assert.equal(
    new Date(i.occurred_at).toISOString(),
    '2026-09-01T16:00:00.000Z',
    'occurred_at = completed_at',
  )
  assert.equal(i.person_id, personId, 'person copied from the showing row')
  assert.equal(i.property_id, propertyId, 'property copied from the showing row')
  assert.equal(i.deal_id, dealId, 'deal copied from the showing row')
  assert.equal(i.source_system, 'showing')
  assert.equal(i.source_external_id, created.id, 'idempotency keyed by showing.id')

  // Replay: completing the already-completed showing is a no-op — the
  // interaction is never duplicated and completed_at is not overwritten.
  await completeShowing(created.id, '2026-09-01T17:00:00.000Z')
  const afterReplay = await showingInteractions(created.id)
  assert.equal(afterReplay.length, 1, 'replay does not duplicate the interaction')
  const row = (await sql`select status, completed_at from showing where id = ${created.id}`)[0] as Row
  assert.equal(row.status, 'completed')
  assert.equal(
    new Date(row.completed_at).toISOString(),
    '2026-09-01T16:00:00.000Z',
    'completed_at unchanged on replay',
  )
})

test('CRM-11: requested/scheduled/cancelled showing transitions emit no timeline interaction (real Postgres)', async () => {
  const personId = await createFixturePerson()

  const requested = await createShowing({ personId })
  fixture.showingIds.push(requested.id)
  assert.equal((await showingInteractions(requested.id)).length, 0, 'requested emits none')

  await scheduleShowing(requested.id, '2026-09-02T10:00:00.000Z')
  assert.equal((await showingInteractions(requested.id)).length, 0, 'scheduled emits none')

  await cancelShowing(requested.id)
  assert.equal((await showingInteractions(requested.id)).length, 0, 'cancelled emits none')

  const person2 = await createFixturePerson()
  const cancelledDirect = await createShowing({ personId: person2 })
  fixture.showingIds.push(cancelledDirect.id)
  await cancelShowing(cancelledDirect.id)
  assert.equal((await showingInteractions(cancelledDirect.id)).length, 0, 'direct cancel emits none')
})

test('CRM-11: request_source_interaction_id persists when the showing originates from a viewing-request interaction (real Postgres)', async () => {
  const personId = await createFixturePerson()
  const sourceInteractionId = await createFixtureSourceInteraction(personId)

  const linked = await createShowing({ personId, requestSourceInteractionId: sourceInteractionId })
  fixture.showingIds.push(linked.id)
  const linkedRow = (await sql`select request_source_interaction_id from showing where id = ${linked.id}`)[0] as Row
  assert.equal(linkedRow.request_source_interaction_id, sourceInteractionId, 'source interaction recorded')

  const unlinked = await createShowing({ personId })
  fixture.showingIds.push(unlinked.id)
  const unlinkedRow = (await sql`select request_source_interaction_id from showing where id = ${unlinked.id}`)[0] as Row
  assert.equal(unlinkedRow.request_source_interaction_id, null, 'null when no source interaction')
})

test('CRM-11: counter offers are new submitted rows linked via parent_offer_id; no countered status (real Postgres)', async () => {
  const personId = await createFixturePerson()
  const propertyId = await createFixtureProperty()
  const dealId = await createFixtureDeal(personId, propertyId)

  const original = await submitOffer({ dealId, personId, amount: 500000 })
  fixture.offerIds.push(original.offerId)

  const counter = await submitOffer({ dealId, personId, amount: 525000, parentOfferId: original.offerId })
  fixture.offerIds.push(counter.offerId)

  const orig = (await sql`select status, parent_offer_id from offer where id = ${original.offerId}`)[0] as Row
  assert.equal(orig.status, 'submitted')
  assert.equal(orig.parent_offer_id, null, 'original offer has no parent')

  const cnt = (await sql`select status, parent_offer_id from offer where id = ${counter.offerId}`)[0] as Row
  assert.equal(cnt.status, 'submitted', 'counter is a new submitted row')
  assert.equal(cnt.parent_offer_id, original.offerId, 'lineage via parent pointer')

  // The schema forbids a 'countered' status value outright (no invented status).
  await assert.rejects(
    () => sql`insert into offer (deal_id, person_id, amount, status) values (${dealId}, ${personId}, 1, 'countered')`,
    /violates check constraint/,
  )
})

test('CRM-11: submitOffer rejects a parent offer from a different deal (real Postgres)', async () => {
  const personId = await createFixturePerson()
  const p1 = await createFixtureProperty()
  const d1 = await createFixtureDeal(personId, p1)
  const p2 = await createFixtureProperty()
  const d2 = await createFixtureDeal(personId, p2)

  const original = await submitOffer({ dealId: d1, personId, amount: 400000 })
  fixture.offerIds.push(original.offerId)

  await assert.rejects(
    () => submitOffer({ dealId: d2, personId, amount: 410000, parentOfferId: original.offerId }),
    (err: any) => err?.code === 'validation',
  )
})

test('CRM-11: withdrawOffer and rejectOffer transition only submitted offers and never touch deal.stage (real Postgres)', async () => {
  const personId = await createFixturePerson()
  const propertyId = await createFixtureProperty()
  const dealId = await createFixtureDeal(personId, propertyId)

  const o1 = await submitOffer({ dealId, personId, amount: 300000 })
  fixture.offerIds.push(o1.offerId)
  const withdrawn = await withdrawOffer(o1.offerId)
  assert.equal(withdrawn.status, 'withdrawn')
  await assert.rejects(() => withdrawOffer(o1.offerId), (err: any) => err?.code === 'conflict')

  const o2 = await submitOffer({ dealId, personId, amount: 310000 })
  fixture.offerIds.push(o2.offerId)
  const rejected = await rejectOffer(o2.offerId)
  assert.equal(rejected.status, 'rejected')
  await assert.rejects(() => rejectOffer(o2.offerId), (err: any) => err?.code === 'conflict')

  const stage = (await sql`select stage from deal where id = ${dealId}`)[0] as Row
  assert.equal(stage.stage, 'offer', 'portal offer writes never auto-advance the deal stage')
})
