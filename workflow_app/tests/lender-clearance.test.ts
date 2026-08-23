import { test } from 'node:test'
import assert from 'node:assert/strict'
import { lenderClearToCloseFromFact } from '../lender-clearance'
import { setDealLenderClearToClose } from '../../db/deal-lender-clearance'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CRM-20 — lender clear-to-close resolution.
//
// The application fact (lenderClearToClose) is derived from the canonical
// deal.lender_clear_to_close column via the pure mapping, and recorded by the
// explicit application command deal.set_lender_clear_to_close. No database, no
// packages.
// ---------------------------------------------------------------------------

test('lenderClearToCloseFromFact maps all three canonical values', () => {
  assert.equal(lenderClearToCloseFromFact(true), true)
  assert.equal(lenderClearToCloseFromFact(false), false)
  assert.equal(lenderClearToCloseFromFact(null), null)
  assert.equal(lenderClearToCloseFromFact(undefined as unknown as boolean | null), null)
})

// Minimal fake for setDealLenderClearToClose (receipt claim-first + deal update).
type Row = Record<string, any>

class LenderClearanceFake {
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
    if (t.includes('update deal set lender_clear_to_close =') && t.includes('returning id')) {
      const row = this.deals.find((d) => d.id === p[1])
      if (!row) return Promise.resolve([])
      row.lender_clear_to_close = p[0]
      return Promise.resolve([{ id: row.id }])
    }
    throw new Error(`LENDER_CLEARANCE_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('setDealLenderClearToClose writes the canonical fact and is idempotent', async () => {
  const f = new LenderClearanceFake()
  f.deals.push({ id: 'deal-1', lender_clear_to_close: null })

  const res = await setDealLenderClearToClose(
    { dealId: 'deal-1', lenderClearToClose: true, commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.lender_clear_to_close, true)

  // Duplicate update with the same commandId replays the winner's outcome and
  // does not double-mutate.
  const replay = await setDealLenderClearToClose(
    { dealId: 'deal-1', lenderClearToClose: true, commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.lender_clear_to_close, true)
})

test('setDealLenderClearToClose rejects a non-boolean value', async () => {
  const f = new LenderClearanceFake()
  const bad = await setDealLenderClearToClose(
    { dealId: 'deal-1', lenderClearToClose: 'yes' as any, commandId: 'cmd-2' },
    f.runner,
  )
  assert.equal(bad.outcome, 'validation_failure')
})

test('setDealLenderClearToClose returns not_found for a missing deal', async () => {
  const f = new LenderClearanceFake()
  const missing = await setDealLenderClearToClose(
    { dealId: 'nope', lenderClearToClose: false, commandId: 'cmd-3' },
    f.runner,
  )
  assert.equal(missing.outcome, 'not_found')
})
