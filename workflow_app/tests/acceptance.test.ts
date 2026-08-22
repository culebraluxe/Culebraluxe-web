import { test } from 'node:test'
import assert from 'node:assert/strict'
import { acceptOffer } from '../../db/offer-acceptance'
import { setDealStage } from '../../db/deal-stage'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// Minimal in-memory fake for the canonical domain tables used by acceptOffer
// and setDealStage, modeling the claim-first receipt SQL. No database, no
// packages.

type Row = Record<string, any>

class FakeDomain {
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
    if (t.includes('select') && t.includes('workflow_command_receipt') && t.includes('where command_id')) {
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
    if (t.includes('update deal set stage =') && t.includes('returning id, stage')) {
      const row = this.deals.find((d) => d.id === p[2] && d.stage === p[3])
      if (!row) return Promise.resolve([])
      row.stage = p[0]
      return Promise.resolve([{ id: row.id, stage: row.stage }])
    }
    if (t.includes('select stage from deal where id =')) {
      const row = this.deals.find((d) => d.id === p[0])
      return Promise.resolve(row ? [{ stage: row.stage }] : [])
    }

    throw new Error(`FAKE_DOMAIN_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('acceptOffer accepts a submitted offer and preserves competing offers', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push(
    { id: 'offer-1', deal_id: 'deal-1', status: 'submitted' },
    { id: 'offer-2', deal_id: 'deal-1', status: 'submitted' },
  )

  const res = await acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'success')
  assert.equal(res.aggregateId, 'offer-1')
  assert.equal(f.offers.find((o) => o.id === 'offer-1')!.status, 'accepted')
  assert.equal(f.offers.find((o) => o.id === 'offer-2')!.status, 'submitted', 'competing offer preserved')
})

test('second accepted offer is rejected with conflict', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push(
    { id: 'offer-1', deal_id: 'deal-1', status: 'accepted' },
    { id: 'offer-2', deal_id: 'deal-1', status: 'submitted' },
  )

  const res = await acceptOffer({ dealId: 'deal-1', offerId: 'offer-2', commandId: 'cmd-2' }, f.runner)
  assert.equal(res.outcome, 'conflict')
  assert.equal(f.offers.find((o) => o.id === 'offer-2')!.status, 'submitted')
})

test('duplicate accept commandId replays success without re-accepting', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push({ id: 'offer-1', deal_id: 'deal-1', status: 'submitted' })

  const first = await acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, f.runner)
  assert.equal(first.outcome, 'success')

  const replay = await acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, f.runner)
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('acceptOffer requires the offer to belong to the deal', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.offers.push({ id: 'offer-1', deal_id: 'deal-2', status: 'submitted' })

  const res = await acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'validation_failure')
})

test('setDealStage moves offer -> under_contract', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'offer' })

  const res = await setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.stage, 'under_contract')
})

test('setDealStage stale stage returns conflict', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'closed' })

  const res = await setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'conflict')
})

test('setDealStage under_contract -> closed succeeds and duplicate replays', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealStage({ dealId: 'deal-1', from: 'under_contract', to: 'closed', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.stage, 'closed')

  const replay = await setDealStage({ dealId: 'deal-1', from: 'under_contract', to: 'closed', commandId: 'cmd-1' }, f.runner)
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('setDealStage rejects arbitrary transitions', async () => {
  const f = new FakeDomain()
  f.deals.push({ id: 'deal-1', stage: 'new_lead' })

  const res = await setDealStage({ dealId: 'deal-1', from: 'new_lead', to: 'closed', commandId: 'cmd-1' }, f.runner)
  assert.equal(res.outcome, 'validation_failure')
})
