import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import { describeClaimBlocker } from '@/legacy/workflow_app/forge/forge-claim-blocker'

// ---------------------------------------------------------------------------
// A REFUSED CLAIM MUST EXPLAIN ITSELF.
//
// The engine allows ONE system-wide single-active work item, so a claim fails while a peer
// is working — or while a dead worker's claim sits there. Both used to be the same opaque
// message ("could not claim agent work item"), which cost a manual database query to
// diagnose, and `forge:clean` deliberately will not clear a claim younger than its cutoff.
// These cases fence the explanation and the advice.
// ---------------------------------------------------------------------------

const NOW = Date.parse('2026-09-13T23:30:00Z')
const STALE_MS = 10 * 60_000

const row = (over: Partial<Parameters<typeof describeClaimBlocker>[0]['rows'][number]> = {}) => ({
  id: 'wi-1',
  storyId: 'SOME-STORY-01',
  role: 'lead',
  state: 'Claimed',
  claimedBy: 'worker-7',
  updatedAt: new Date(NOW - 60_000).toISOString(),
  ...over,
})

test('claim blocker: no holder left means the refusal was transient, and says so', () => {
  const why = describeClaimBlocker({ rows: [], nowMs: NOW, staleMs: STALE_MS, storyId: 'MINE-01' })
  assert.match(why, /no work item is holding the single-active lock right now/)
  assert.match(why, /retry/i)
})

test('claim blocker: a STALE holder is named, marked, and points at forge:clean', () => {
  const why = describeClaimBlocker({
    rows: [row({ updatedAt: new Date(NOW - 42 * 60_000).toISOString() })],
    nowMs: NOW,
    staleMs: STALE_MS,
    storyId: 'MINE-01',
  })
  assert.match(why, /wi-1/)
  assert.match(why, /42m old/)
  assert.match(why, /STALE/)
  assert.match(why, /pnpm forge:clean/)
})

test('claim blocker: a fresh holder on ANOTHER story is a live peer, not a leftover', () => {
  const why = describeClaimBlocker({
    rows: [row({ storyId: 'OTHER-STORY-01' })],
    nowMs: NOW,
    staleMs: STALE_MS,
    storyId: 'MINE-01',
  })
  assert.match(why, /live peer/i)
  assert.doesNotMatch(why, /STALE/)
})

test('claim blocker: a fresh claim by THIS story names the reset command for it', () => {
  const why = describeClaimBlocker({
    rows: [row({ storyId: 'MINE-01' })],
    nowMs: NOW,
    staleMs: STALE_MS,
    storyId: 'MINE-01',
  })
  assert.match(why, /pnpm forge:story:reset MINE-01 reset --force/)
})

test('claim blocker: an unreadable timestamp is "age unknown", never a fabricated age', () => {
  const why = describeClaimBlocker({
    rows: [row({ updatedAt: null })],
    nowMs: NOW,
    staleMs: STALE_MS,
    storyId: 'MINE-01',
  })
  assert.match(why, /age unknown/)
  assert.doesNotMatch(why, /NaN/)
  assert.doesNotMatch(why, /STALE/)
})

test('claim blocker: several holders are all named, not just the first', () => {
  const why = describeClaimBlocker({
    rows: [row({ id: 'wi-1' }), row({ id: 'wi-2', storyId: 'OTHER-02' })],
    nowMs: NOW,
    staleMs: STALE_MS,
    storyId: 'MINE-01',
  })
  assert.match(why, /wi-1/)
  assert.match(why, /wi-2/)
})

// --- the wiring fence -------------------------------------------------------
//
// The retry is the point: a refused claim runs the engine's own recovery (stale-only) and
// tries again before it gives up.

const RUNNER = readFileSync(
  new URL('../forge/agent-runtime-role-runner.ts', import.meta.url),
  'utf8',
)

test('claim blocker: a refused claim recovers stale claims and retries ONCE before refusing', () => {
  const claimAt = RUNNER.indexOf('let claimed = await work.claimSpecific(')
  const recoverAt = RUNNER.indexOf('recoverStaleForgeEngineClaims({ limit: 20 })')
  // Search AFTER the recovery: the first claim line contains the same substring ('let claimed = ...').
  const retryAt = RUNNER.indexOf('claimed = await work.claimSpecific(queued.id, options.workerId)', recoverAt)
  // From the retry onward: the phrase also appears in the explaining comment above the claim.
  const throwAt = RUNNER.indexOf('could not claim agent work item', retryAt)

  assert.ok(claimAt > 0, 'the runner must claim')
  assert.ok(recoverAt > claimAt, 'recovery happens only AFTER a refusal')
  assert.ok(retryAt > recoverAt, 'the retry follows the recovery')
  assert.ok(throwAt > retryAt, 'the refusal is the last resort, after the retry')
  assert.match(
    RUNNER,
    /recovered\.filter\(\(outcome\) => outcome\.recovered\)\.length/,
    'the recovery is counted, so the message can say what it cleared',
  )
  assert.match(RUNNER, /describeClaimBlocker\(\{/, 'the refusal names the holder')
  assert.match(
    RUNNER,
    /staleMs: DEFAULT_FORGE_STALE_MS/,
    'the advice must use the same staleness cutoff the sweep uses',
  )
})
