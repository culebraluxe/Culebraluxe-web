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

// THE CAS MUST REPORT WHAT IT DID.
//
// `recoverStaleForgeEngineClaims` gates its work-item release on `updated.length` from the
// CAS update. Without a RETURNING clause that row set is always empty, so every recovery
// returned `{ recovered: false, reason: 'cas-miss' }` — while still interrupting the row —
// and the work item was never released. Live on 2026-09-13 a sweep reported "0 recovered,
// 15 skipped" seconds after stamping all fifteen rows 'stale claim recovered'.
//
// The source is read here because the behaviour needs a live database; the fence is that
// the statement which decides the outcome must be able to report it.
test('ENG-FORGE-RECOVERY: the stale-claim CAS carries RETURNING, or it lies about itself', async () => {
  const { readFileSync } = await import('node:fs')
  const source = readFileSync(new URL('../../db/forge-engine-recovery.ts', import.meta.url), 'utf8')

  const cas = source.slice(source.indexOf("set status = 'interrupted', last_error = 'stale claim recovered'"))
  const statement = cas.slice(0, cas.indexOf('`', cas.indexOf('where task_id')))

  assert.match(
    statement,
    /returning\s+task_id/i,
    'the outcome check reads updated.length, so the UPDATE must return the rows it changed',
  )
  assert.match(
    source,
    /if\s*\(!updated\.length\)/,
    'the guard this fence protects must still exist, or the fence is stale',
  )
})

