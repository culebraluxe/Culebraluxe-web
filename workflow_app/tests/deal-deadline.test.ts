import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  setDealMilestoneDeadline,
  DEAL_DEADLINE_COLUMNS,
  isDealMilestoneKey,
} from '../../db/deal-deadline'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CRM-22 — canonical deal milestone deadline service (pure, fake DB).
//
// The service owns legality: milestone whitelist (never an arbitrary column),
// date validity, claim-first idempotent receipt, no unrelated deal mutation.
// ---------------------------------------------------------------------------

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
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      // finalizeReceipt passes command_id as its LAST parameter; the receipt
      // shape differs by actor threading (4 vs 5 params), so look up by the
      // trailing parameter with a fallback to stay robust to both shapes.
      const commandId = p[4] ?? p[3]
      const r = this.receipts.find((x) => x.command_id === commandId)
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
        r.actor_app_user_id = p[3] ?? null
      }
      return Promise.resolve([])
    }
    if (t.includes('from workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }] : [])
    }
    const deadlineUpdate = t.match(/update deal set (inspection_deadline|financing_deadline) = \$1::date, updated_at = now\(\) where id = \$2 returning id/)
    if (deadlineUpdate) {
      const column = deadlineUpdate[1]
      const row = this.deals.find((d) => d.id === p[1])
      if (!row) return Promise.resolve([])
      row[column] = p[0]
      return Promise.resolve([{ id: row.id }])
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('CRM-22: milestone whitelist covers exactly the canonical deadline columns', () => {
  assert.deepEqual(DEAL_DEADLINE_COLUMNS, {
    inspection: 'inspection_deadline',
    financing: 'financing_deadline',
  })
  assert.equal(isDealMilestoneKey('inspection'), true)
  assert.equal(isDealMilestoneKey('financing'), true)
  assert.equal(isDealMilestoneKey('appraisal'), false)
  assert.equal(isDealMilestoneKey('title_work'), false)
})

test('deal.set_inspection_deadline writes the canonical inspection deadline', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'inspection', deadline: '2026-09-15', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.inspection_deadline, '2026-09-15')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.financing_deadline, undefined)
})

test('deal.set_financing_deadline writes the canonical financing deadline', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'financing', deadline: '2026-10-01', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.financing_deadline, '2026-10-01')
})

test('unknown milestone is rejected (no arbitrary column write)', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'appraisal' as any, deadline: '2026-09-15', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'validation_failure')
  assert.match(res.message ?? '', /milestone/)
  assert.equal(f.receipts.length, 0, 'no receipt is claimed for a rejected milestone')
})

test('invalid date is rejected', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const res = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'inspection', deadline: 'not-a-date', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'validation_failure')
  assert.match(res.message ?? '', /valid date/)
})

test('duplicate commandId replays without re-writing', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })

  const first = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'inspection', deadline: '2026-09-15', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(first.outcome, 'success')

  const replay = await setDealMilestoneDeadline(
    { dealId: 'deal-1', milestone: 'inspection', deadline: '2026-09-15', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('missing deal returns not_found', async () => {
  const f = new FakeDb()

  const res = await setDealMilestoneDeadline(
    { dealId: 'missing', milestone: 'inspection', deadline: '2026-09-15', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'not_found')
})
