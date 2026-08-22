import { test } from 'node:test'
import assert from 'node:assert/strict'
import { appraisalApplicableFromRequired } from '../appraisal'
import { setDealAppraisalRequired } from '../../db/deal-appraisal'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CRM-19 — appraisal applicability resolution.
//
// The application fact (appraisalApplicable) is derived from the canonical
// deal.appraisal_required column via the pure mapping, and resolved by the
// explicit application command deal.set_appraisal_required. No database, no
// packages.
// ---------------------------------------------------------------------------

test('appraisalApplicableFromRequired maps all three canonical values', () => {
  assert.equal(appraisalApplicableFromRequired(true), true)
  assert.equal(appraisalApplicableFromRequired(false), false)
  assert.equal(appraisalApplicableFromRequired(null), null)
  assert.equal(appraisalApplicableFromRequired(undefined as unknown as boolean | null), null)
})

// Minimal fake for setDealAppraisalRequired (receipt claim-first + deal update).
type Row = Record<string, any>

class AppraisalFake {
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
    if (t.includes('select') && t.includes('workflow_command_receipt') && t.includes('where command_id')) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message }] : [])
    }
    if (t.includes('update deal set appraisal_required =') && t.includes('returning id')) {
      const row = this.deals.find((d) => d.id === p[1])
      if (!row) return Promise.resolve([])
      row.appraisal_required = p[0]
      return Promise.resolve([{ id: row.id }])
    }
    throw new Error(`APPRAISAL_UNHANDLED: ${t}`)
  }

  runner: TxRunner = (cb) => cb(this.tx)
}

test('setDealAppraisalRequired writes the canonical fact and is idempotent', async () => {
  const f = new AppraisalFake()
  f.deals.push({ id: 'deal-1', appraisal_required: null })

  const res = await setDealAppraisalRequired(
    { dealId: 'deal-1', appraisalRequired: true, commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(res.outcome, 'success')
  assert.equal(f.deals.find((d) => d.id === 'deal-1')!.appraisal_required, true)

  const replay = await setDealAppraisalRequired(
    { dealId: 'deal-1', appraisalRequired: true, commandId: 'cmd-1' },
    f.runner,
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
})

test('setDealAppraisalRequired rejects a non-boolean value', async () => {
  const f = new AppraisalFake()
  const bad = await setDealAppraisalRequired(
    { dealId: 'deal-1', appraisalRequired: 'yes' as any, commandId: 'cmd-2' },
    f.runner,
  )
  assert.equal(bad.outcome, 'validation_failure')
})

test('setDealAppraisalRequired returns not_found for a missing deal', async () => {
  const f = new AppraisalFake()
  const missing = await setDealAppraisalRequired(
    { dealId: 'nope', appraisalRequired: false, commandId: 'cmd-3' },
    f.runner,
  )
  assert.equal(missing.outcome, 'not_found')
})
