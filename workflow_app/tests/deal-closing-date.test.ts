import { test } from 'node:test'
import assert from 'node:assert/strict'
import { setDealClosingDate } from '../../db/deal-closing-date'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

type Row = Record<string, any>

class FakeDb {
  deals: Row[] = []
  receipts: Row[] = []

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
    )
    const p = params as any[]

    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      const exists = this.receipts.some((r) => r.command_id === p[0])
      if (exists) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[4])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
      }
      return Promise.resolve([])
    }
    if (t.includes('from workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: null }] : [])
    }
    if (t.includes('update deal set closing_date =') && t.includes('returning id')) {
      const row = this.deals.find((d) => d.id === p[1])
      if (!row) return Promise.resolve([])
      row.closing_date = p[0]
      return Promise.resolve([{ id: row.id }])
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('deal.set_closing_date writes the canonical target closing date', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealClosingDate(
    { dealId: 'deal-1', closingDate: '2026-09-01', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.closing_date, '2026-09-01')
})

test('deal.set_closing_date rejects an invalid date', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealClosingDate(
    { dealId: 'deal-1', closingDate: 'not-a-date', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'validation_failure')
})

test('duplicate deal.set_closing_date commandId replays without re-writing', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const first = await setDealClosingDate(
    { dealId: 'deal-1', closingDate: '2026-09-01', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(first.outcome, 'success')

  const replay = await setDealClosingDate(
    { dealId: 'deal-1', closingDate: '2026-09-01', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('deal.set_closing_date returns not_found for a missing deal', async () => {
  const f = new FakeDb()

  const res = await setDealClosingDate(
    { dealId: 'missing', closingDate: '2026-09-01', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'not_found')
})
