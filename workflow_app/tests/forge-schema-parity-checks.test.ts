// ---------------------------------------------------------------------------
// FORGE-PARITY-CHECK-01 — the fifth parity axis: CHECK constraints.
//
// The drift this closes, measured on 2026-09-12: `pnpm db:parity` reported the
// control-plane databases identical while PROD enforced 138 check constraints and
// DEV enforced 137. The missing one in DEV was
//
//   agent_work_item.agent_work_item_parallel_shape_check
//
// i.e. PROD refused a malformed split lane that DEV accepted — the dangerous
// direction, because a worker passes in DEV and fails in PROD. The gate could not
// see it because it never read pg_constraint for contype='c'.
//
// These tests are pure: compareSnapshots takes snapshots, not databases.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import { checkValue, compareSnapshots, type SchemaSnapshot } from '../../lib/schema-parity'

const PARALLEL_SHAPE =
  "CHECK (((parallel_group_id IS NULL) AND (parallel_slot IS NULL)) OR ((parallel_group_id IS NOT NULL) AND (lane = 'smith'::text)))"

const snap = (over: Partial<SchemaSnapshot> = {}): SchemaSnapshot => ({
  tables: ['agent_work_item'],
  columns: new Map([['agent_work_item', new Map([['id', 'text NOT NULL']])]]),
  indexes: new Map(),
  fks: new Map(),
  checks: new Map([['agent_work_item.agent_work_item_parallel_shape_check', PARALLEL_SHAPE]]),
  ...over,
})

describe('check constraints are compared', () => {
  it('the real 2026-09-12 finding: PROD-only check is drift, not clean', () => {
    const report = compareSnapshots(snap({ checks: new Map() }), snap())
    assert.equal(report.checkDrift.length, 1)
    assert.match(report.checkDrift[0], /agent_work_item_parallel_shape_check: DEV=- PROD=CHECK/)
    assert.equal(report.clean, false, 'a PROD constraint DEV lacks must fail the release gate')
  })

  it('DEV-only check is drift too (the direction that hides a bad publish)', () => {
    const report = compareSnapshots(snap(), snap({ checks: new Map() }))
    assert.equal(report.checkDrift.length, 1)
    assert.match(report.checkDrift[0], /DEV=CHECK .*PROD=-/)
    assert.equal(report.clean, false)
  })

  it('identical checks are clean', () => {
    const report = compareSnapshots(snap(), snap())
    assert.deepEqual(report.checkDrift, [])
    assert.equal(report.clean, true)
  })

  it('same name, different definition is drift — and both sides are visible', () => {
    const report = compareSnapshots(
      snap({ checks: new Map([['t.c', 'CHECK ((a > 0))']]) }),
      snap({ checks: new Map([['t.c', 'CHECK ((a > 10))']]) }),
    )
    assert.equal(report.checkDrift.length, 1)
    assert.match(report.checkDrift[0], /DEV=CHECK \(\(a > 0\)\) PROD=CHECK \(\(a > 10\)\)/)
  })

  it('NOT VALID is not equal to validated', () => {
    const report = compareSnapshots(
      snap({ checks: new Map([['t.c', checkValue('CHECK ((a > 0))', false)]]) }),
      snap({ checks: new Map([['t.c', checkValue('CHECK ((a > 0))', true)]]) }),
    )
    assert.equal(report.checkDrift.length, 1, 'a constraint that does not police existing rows is weaker')
    assert.match(report.checkDrift[0], /NOT VALID/)
    assert.equal(report.clean, false)
  })

  it('checkValue normalizes the definition Postgres pretty-prints across lines', () => {
    const oneLine = checkValue('CHECK ((a > 0))', true)
    const wrapped = checkValue('CHECK ((a >\n   0))', true)
    assert.equal(wrapped, oneLine)
    assert.equal(checkValue('CHECK ((a > 0))', false), `${oneLine} NOT VALID`)
  })

  it('the other four axes still behave (a check-only change does not mask them)', () => {
    const report = compareSnapshots(
      snap({ tables: ['agent_work_item', 'extra_dev'] }),
      snap(),
    )
    assert.deepEqual(report.tablesOnlyDev, ['extra_dev'])
    assert.deepEqual(report.checkDrift, [])
    assert.equal(report.clean, false)
  })
})
