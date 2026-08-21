import { test } from 'node:test'
import assert from 'node:assert/strict'
import { acceptOffer } from '../../db/offer-acceptance'
import { setDealStage } from '../../db/deal-stage'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// Models the PostgreSQL claim-first serialization boundary: UNIQUE(command_id)
// blocks a losing concurrent INSERT until the winner commits or rolls back.
type Row = Record<string, any>
type Receipt = {
  outcome: string
  aggregateId: string | null
  message: string | null
  settled: boolean
  waiters: Array<() => void>
}

class ConcurrencyDomain {
  receipts = new Map<string, Receipt>()
  deals = new Map<string, Row>()
  offers = new Map<string, Row>()
  offerEffectCount = 0
  stageEffectCount = 0
  failNextEffect = false
  joinState = new Map<string, Set<string>>()
  tokens = new Map<string, string>()

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  async claim(commandId: string): Promise<boolean> {
    const existing = this.receipts.get(commandId)
    if (!existing) {
      this.receipts.set(commandId, {
        outcome: 'pending',
        aggregateId: null,
        message: null,
        settled: false,
        waiters: [],
      })
      return true
    }
    if (!existing.settled) {
      await new Promise<void>((res) => existing.waiters.push(res))
    }
    return false
  }

  finalize(
    commandId: string,
    outcome: string,
    aggregateId: string | null,
    message: string | null,
  ) {
    const r = this.receipts.get(commandId)!
    r.outcome = outcome
    r.aggregateId = aggregateId
    r.message = message
    r.settled = true
    const w = r.waiters
    r.waiters = []
    w.forEach((res) => res())
  }

  rollbackInFlight() {
    for (const [k, v] of this.receipts) {
      if (!v.settled) this.receipts.delete(k)
    }
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
    )
    const p = params as any[]

    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      return this.claim(p[0]).then((won) => (won ? [{ command_id: p[0] }] : []))
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      this.finalize(p[3], p[0], p[1], p[2])
      return Promise.resolve([])
    }
    if (t.includes('select') && t.includes('workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.get(p[0])
      return Promise.resolve(
        r && r.settled
          ? [{ command_id: p[0], outcome: r.outcome, aggregate_id: r.aggregateId, message: r.message }]
          : [],
      )
    }

    if (t.includes('select id from deal where id =') && t.includes('for update')) {
      const row = this.deals.get(p[0])
      return Promise.resolve(row ? [{ id: row.id }] : [])
    }
    if (t.includes('select id, deal_id, status from offer where id =') && t.includes('for update')) {
      const row = this.offers.get(p[0])
      return Promise.resolve(row ? [{ id: row.id, deal_id: row.deal_id, status: row.status }] : [])
    }
    if (t.includes("select id from offer where deal_id =") && t.includes("status = 'accepted'")) {
      const rows = [...this.offers.values()].filter(
        (o) => o.deal_id === p[0] && o.status === 'accepted',
      )
      return Promise.resolve(rows.map((r) => ({ id: r.id })))
    }
    if (t.includes('update offer set status =') && t.includes("'accepted'")) {
      if (this.failNextEffect) {
        this.failNextEffect = false
        throw new Error('simulated offer effect failure')
      }
      const row = this.offers.get(p[0])
      if (row && row.status === 'submitted') {
        row.status = 'accepted'
        this.offerEffectCount++
      }
      return Promise.resolve([])
    }
    if (t.includes('update deal set stage =') && t.includes('returning id, stage')) {
      if (this.failNextEffect) {
        this.failNextEffect = false
        throw new Error('simulated stage effect failure')
      }
      const row = this.deals.get(p[2])
      if (!row || row.stage !== p[3]) return Promise.resolve([])
      row.stage = p[0]
      this.stageEffectCount++
      return Promise.resolve([{ id: row.id, stage: row.stage }])
    }
    if (t.includes('select stage from deal where id =')) {
      const row = this.deals.get(p[0])
      return Promise.resolve(row ? [{ stage: row.stage }] : [])
    }

    if (t.includes('insert into join_state') && t.includes('on conflict do nothing')) {
      const joinId = p[0] as string
      const branchId = p[1] as string
      const set = this.joinState.get(joinId) ?? new Set<string>()
      if (set.has(branchId)) return Promise.resolve([])
      set.add(branchId)
      this.joinState.set(joinId, set)
      return Promise.resolve([{ branch_id: branchId }])
    }
    if (t.includes('select count(*) from join_state') && t.includes('where join_id')) {
      const joinId = p[0] as string
      const set = this.joinState.get(joinId)
      const count = set ? set.size : 0
      return Promise.resolve([{ count }])
    }
    if (t.includes('insert into token') && t.includes('on conflict do nothing')) {
      const tokenId = p[0] as string
      const joinId = p[1] as string
      // ensure only one token per join
      if ([...this.tokens.values()].includes(joinId)) return Promise.resolve([])
      this.tokens.set(tokenId, joinId)
      return Promise.resolve([{ token_id: tokenId }])
    }

    throw new Error(`CONCURRENCY_UNHANDLED: ${t}`)
  }

  runner: TxRunner = async (cb) => {
    try {
      return await cb(this.tx)
    } catch (e) {
      this.rollbackInFlight()
      throw e
    }
  }
}

test('same commandId concurrent acceptOffer: one effect, both observe success', async () => {
  const d = new ConcurrencyDomain()
  d.deals.set('deal-1', { id: 'deal-1' })
  d.offers.set('offer-1', { id: 'offer-1', deal_id: 'deal-1', status: 'submitted' })

  const [r1, r2] = await Promise.all([
    acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, d.runner),
    acceptOffer({ dealId: 'deal-1', offerId: 'offer-1', commandId: 'cmd-1' }, d.runner),
  ])

  assert.equal(r1.outcome, 'success')
  assert.equal(r2.outcome, 'success', 'both callers observe the same logical result')
  assert.equal(d.offerEffectCount, 1, 'business effect happened exactly once')
  assert.equal(d.receipts.size, 1)
  assert.equal(d.receipts.get('cmd-1')!.outcome, 'success')
})

test('same commandId concurrent setDealStage: one effect, both observe success', async () => {
  const d = new ConcurrencyDomain()
  d.deals.set('deal-1', { id: 'deal-1', stage: 'offer' })

  const [r1, r2] = await Promise.all([
    setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, d.runner),
    setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, d.runner),
  ])

  assert.equal(r1.outcome, 'success')
  assert.equal(r2.outcome, 'success')
  assert.equal(d.stageEffectCount, 1)
  assert.equal(d.receipts.size, 1)
})

test('rolled-back attempt does not poison a future retry', async () => {
  const d = new ConcurrencyDomain()
  d.deals.set('deal-1', { id: 'deal-1', stage: 'offer' })

  d.failNextEffect = true
  await assert.rejects(
    setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, d.runner),
  )
  assert.equal(d.receipts.size, 0, 'pending receipt must roll back with the transaction')

  const res = await setDealStage({ dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' }, d.runner)
  assert.equal(res.outcome, 'success')
  assert.equal(res.replayed, false)
  assert.equal(d.stageEffectCount, 1)
})

test('two required fork branches complete concurrently against the same join: join releases exactly once and produces exactly one downstream token', async () => {
  const d = new ConcurrencyDomain()
  const joinId = 'join-1'
  const branchA = 'branch-a'
  const branchB = 'branch-b'

  async function completeBranch(joinId: string, branchId: string) {
    await d.runner(async (tx) => {
      await tx`
        insert into join_state (join_id, branch_id) values (${joinId}, ${branchId})
        on conflict do nothing
        returning branch_id
      `
      const countRows = await tx`
        select count(*) from join_state where join_id = ${joinId}
      `
      const count = Number(countRows[0]?.count ?? 0)
      if (count === 2) {
        await tx`
          insert into token (token_id, join_id) values (${`token-${joinId}`}, ${joinId})
          on conflict do nothing
          returning token_id
        `
      }
    })
  }

  await Promise.all([
    completeBranch(joinId, branchA),
    completeBranch(joinId, branchB),
  ])

  assert.equal(d.joinState.get(joinId)?.size, 2)
  const tokensForJoin = [...d.tokens.entries()].filter(([, j]) => j === joinId)
  assert.equal(tokensForJoin.length, 1)
  assert.ok(tokensForJoin[0])
})
