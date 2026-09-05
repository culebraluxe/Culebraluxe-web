import assert from 'node:assert/strict'
import test from 'node:test'

import { isStaleHeartbeat } from '../../db/forge-engine-recovery'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S3 — staleness predicate (pure). The full recovery path is a
// real-DB operation exercised in the persistence suite.
// ---------------------------------------------------------------------------

const NOW = new Date('2026-09-04T00:00:00Z')

test('ENG-FORGE-V10 S3: null/unknown heartbeat is stale', () => {
  assert.equal(isStaleHeartbeat(null, NOW), true)
  assert.equal(isStaleHeartbeat(undefined, NOW), true)
  assert.equal(isStaleHeartbeat('not-a-date', NOW), true)
})

test('ENG-FORGE-V10 S3: a heartbeat within the threshold is fresh (never stolen)', () => {
  const fresh = new Date(NOW.getTime() - 60_000)
  assert.equal(isStaleHeartbeat(fresh, NOW, 10 * 60_000), false)
})

test('ENG-FORGE-V10 S3: an old heartbeat is stale', () => {
  const old = new Date(NOW.getTime() - 11 * 60_000)
  assert.equal(isStaleHeartbeat(old, NOW, 10 * 60_000), true)
})
