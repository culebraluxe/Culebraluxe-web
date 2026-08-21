import { test } from 'node:test'
import assert from 'node:assert/strict'
import { replayOutcome } from '../../db/workflow-command-receipt'
import { setDealStage } from '../../db/deal-stage'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

type Row = Record<string, any>

// Minimal in-memory fake for the receipt + deal stage surface used by
// setDealStage. No database, no packages.

class FakeDb {
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
      if (this.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[3])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
      }
      return Promise.resolve([])
    }
    if (t.includes('select command_id, outcome, aggregate_id, message from workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message }] : [])
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

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

// ---------------------------------------------------------------------------
// replayOutcome — pending is never a terminal CommandOutcome
// ---------------------------------------------------------------------------

test('a null receipt is an in-flight conflict', () => {
  const r = replayOutcome(null)
  assert.equal(r.outcome, 'conflict')
  assert.match(r.message ?? '', /in-flight/i)
})

test('a pending receipt is an in-flight conflict, not a terminal outcome', () => {
  const r = replayOutcome({ commandId: 'c', outcome: 'pending', aggregateId: null, message: null })
  assert.equal(r.outcome, 'conflict')
  assert.match(r.message ?? '', /pending/i)
  assert.equal(replayOutcome({ commandId: 'c', outcome: 'pending', aggregateId: null, message: null }).outcome, 'conflict')
})

test('a completed receipt replays success deterministically', () => {
  const r = replayOutcome({ commandId: 'c', outcome: 'success', aggregateId: 'a', message: null })
  assert.deepEqual(r, { outcome: 'success', message: null })
})

test('a failed/conflict terminal receipt replays its stored outcome', () => {
  assert.equal(
    replayOutcome({ commandId: 'c', outcome: 'validation_failure', aggregateId: null, message: 'bad' }).outcome,
    'validation_failure',
  )
  assert.equal(
    replayOutcome({ commandId: 'c', outcome: 'conflict', aggregateId: null, message: 'm' }).outcome,
    'conflict',
  )
})

// ---------------------------------------------------------------------------
// setDealStage — a poisoned pending receipt must not re-run the mutation
// ---------------------------------------------------------------------------

test('setDealStage with a pending receipt returns conflict and does NOT mutate the deal', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'offer' })
  f.receipts.push({ command_id: 'cmd-1', outcome: 'pending', aggregate_id: null, message: null })

  const res = await setDealStage(
    { dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'conflict')
  assert.equal(res.replayed, true)
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.stage, 'offer', 'deal must not be mutated on replay of pending')
})

test('setDealStage with a completed receipt replays success without re-mutating', async () => {
  const f = new FakeDb()
  f.deals.push({ id: 'deal-1', stage: 'under_contract' })
  f.receipts.push({ command_id: 'cmd-1', outcome: 'success', aggregate_id: 'deal-1', message: null })

  const res = await setDealStage(
    { dealId: 'deal-1', from: 'offer', to: 'under_contract', commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(res.replayed, true)
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.stage, 'under_contract')
})
