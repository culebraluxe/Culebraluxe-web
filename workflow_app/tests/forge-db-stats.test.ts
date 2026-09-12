// ---------------------------------------------------------------------------
// ForgeDB telemetry — the observability half of DB-HARDEN-02.
//
// No database is required: these pin the CONTRACT an operator relies on, namely
// that asking for stats never creates a connection, never needs a declared
// environment (an explicit target is enough), and never reports NaN when nothing
// has run yet. The live numbers are exercised against PROD separately.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { forgeDb, forgeDbMetrics } from '../../db/forge-db'

const METRIC_KEYS = [
  'queries',
  'queryErrors',
  'queryAvgMs',
  'queryMaxMs',
  'acquisitions',
  'acquireAvgMs',
  'acquireMaxMs',
] as const

test('forge db stats: an explicit target needs no declaration and opens no connection', () => {
  const stats = forgeDb.forTarget('prod').stats()
  assert.equal(stats.target, 'prod')
  assert.equal(stats.created, false, 'asking for stats must not create a pool')
  assert.equal(stats.totalCount, 0)
  assert.equal(stats.idleCount, 0)
  assert.equal(stats.waitingCount, 0)
  for (const key of METRIC_KEYS) {
    assert.ok(key in stats, `stats must include ${key}`)
  }
})

test('forge db metrics: zeroes, never NaN, before anything has run', () => {
  const metrics = forgeDbMetrics()
  for (const key of METRIC_KEYS) {
    const value = metrics[key]
    assert.equal(typeof value, 'number', `${key} must be a number`)
    assert.ok(Number.isFinite(value), `${key} must be finite (no NaN with zero samples)`)
  }
})

test('forge db metrics: a fresh process reports no acquisitions and no errors', () => {
  // This file is its own process, so the counters are untouched here.
  const metrics = forgeDbMetrics()
  assert.equal(metrics.queryErrors, 0)
  assert.equal(metrics.acquisitions, 0)
  assert.equal(metrics.queryMaxMs, 0)
})
