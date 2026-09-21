import assert from 'node:assert/strict'
import test from 'node:test'

import {
  SPEND_SOURCES,
  normalizeSpendSource,
  resolveSpendSource,
} from '@/legacy/db/storyboard'
import { recordForgeRunMachineEvidence } from '@/legacy/db/forge-run'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'
import type { RunMachineEvidence } from '@/lib/forge-run-evidence'

// ENG-FORGE-REVIEW-RESIDUALS-01 Unit B — every run carries a closed spend source.
//
// `cost_source` could stay NULL: the writers only labelled a row when they had a
// quantity, so an unmeasured run read as unknown and the dollars column had no
// rule keeping widgets out. These lock the closed vocabulary (vendor | widgets |
// none), the writer's decision for all three, and the widget path writing only
// cost_widgets.

const evidence = (over: Partial<RunMachineEvidence> = {}): RunMachineEvidence => ({
  baseCommitHash: null,
  commandsTotal: null,
  commandsPassed: null,
  commandsFailed: null,
  testsTotal: null,
  testsPassed: null,
  testsFailed: null,
  policyViolationCount: null,
  failureCode: null,
  evidenceDetail: null,
  ...over,
})

function capture(): { calls: Array<{ sql: string; params: unknown[] }>; q: QueryExecutor } {
  const calls: Array<{ sql: string; params: unknown[] }> = []
  const q: QueryExecutor = (strings, ...params) => {
    calls.push({ sql: strings.join('?'), params })
    return Promise.resolve([] as QueryRow[])
  }
  return { calls, q }
}

test('the spend source vocabulary is closed to vendor, widgets and none', () => {
  assert.deepEqual([...SPEND_SOURCES], ['vendor', 'widgets', 'none'])
  assert.equal(normalizeSpendSource('vendor'), 'vendor')
  assert.equal(normalizeSpendSource('widgets'), 'widgets')
  assert.equal(normalizeSpendSource(null), 'none')
  assert.equal(normalizeSpendSource('anything-else'), 'none')
})

test('a vendor-cost run carries spend source vendor', async () => {
  assert.equal(resolveSpendSource({ costUsd: 12.5 }), 'vendor')
  const { calls, q } = capture()
  await recordForgeRunMachineEvidence('run-1', evidence({ costUsd: 12.5 }), q)
  assert.equal(calls[0].params[12], 12.5, 'the USD quantity goes to cost_usd')
  assert.equal(calls[0].params[13], null, 'no widget quantity is written')
  assert.equal(calls[0].params[14], 'vendor', 'the row is labelled vendor')
})

test('a widget-cost run carries spend source widgets and writes no dollars', async () => {
  assert.equal(resolveSpendSource({ costWidgets: 3.5 }), 'widgets')
  const { calls, q } = capture()
  await recordForgeRunMachineEvidence('run-1', evidence({ costWidgets: 3.5 }), q)
  assert.equal(calls[0].params[12], null, 'the dollars column stays null')
  assert.equal(calls[0].params[13], 3.5, 'the widget quantity goes to cost_widgets')
  assert.equal(calls[0].params[14], 'widgets', 'the row is labelled widgets')
  assert.ok(calls[0].sql.includes('cost_widgets = coalesce'), 'widgets have their own column')
})

test('an unmeasured run records spend source none, never null', async () => {
  assert.equal(resolveSpendSource({}), null)
  const { calls, q } = capture()
  await recordForgeRunMachineEvidence('run-1', evidence(), q)
  assert.equal(calls[0].params[12], null, 'no dollars')
  assert.equal(calls[0].params[13], null, 'no widgets')
  assert.equal(calls[0].params[14], null, 'this write carried no source')
  assert.ok(calls[0].sql.includes("'none'"), 'absence falls back to the recorded fact none')
})
