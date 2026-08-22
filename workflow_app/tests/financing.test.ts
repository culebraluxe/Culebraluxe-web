import { test } from 'node:test'
import assert from 'node:assert/strict'
import { financingApplicableFromType } from '../financing'
import { setDealFinancingType } from '../../db/deal-financing'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

test('financingApplicableFromType maps all three canonical values', () => {
  assert.equal(financingApplicableFromType('financed'), true)
  assert.equal(financingApplicableFromType('cash'), false)
  assert.equal(financingApplicableFromType(null), null)
  assert.equal(financingApplicableFromType(undefined as unknown as string | null), null)
})

// Minimal fake for setDealFinancingType (receipt claim-first + deal update).
type Row = Record<string, any>

class FinancingFake {
  receipts: Row[] = []
  deals: Row[] = []

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
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
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
    if (t.includes('select') && t.includes('workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }] : [])
    }
    if (t.includes('update deal set financing_type =') && t.includes('returning id')) {
      const row = this.deals.find((d) => d.id === p[1])
      if (!row) return Promise.resolve([])
      row.financing_type = p[0]
      return Promise.resolve([{ id: row.id }])
    }
    throw new Error(`FINANCING_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('setDealFinancingType writes the canonical fact and is idempotent', async () => {
  const f = new FinancingFake()
  f.deals.push({ id: 'deal-1', financing_type: null })

  const res = await setDealFinancingType(
    { dealId: 'deal-1', financingType: 'financed', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.financing_type, 'financed')

  const replay = await setDealFinancingType(
    { dealId: 'deal-1', financingType: 'financed', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('setDealFinancingType rejects an unknown type', async () => {
  const f = new FinancingFake()
  const bad = await setDealFinancingType(
    { dealId: 'deal-1', financingType: 'bogus' as any, commandId: 'cmd-2' },
    f.runner,
  )
  assert.equal(bad.outcome, 'validation_failure')
})
