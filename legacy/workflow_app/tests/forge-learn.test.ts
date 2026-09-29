import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  DEFAULT_LEARN_WINDOW_HOURS,
  MAX_LEARN_ITEMS_PER_PASS,
  buildLearnItemInstructions,
  decideLearnItem,
  learnPatternKey,
  learnStoryId,
  learnWindowStart,
  parseLearnAnchor,
  renderLearnPacket,
  type LearnCandidate,
} from '@/lib/forge-learn'
import { learnCandidatesFromFiles, isLearnWindowPath, staleClaimCandidate } from '@/agent-runtime/learn-loop'
import { listOpenLearnPatternKeys, openReadyLearnItem, stageLearnStory } from '@/legacy/db/forge-learn'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-FACTORY-01 PHASE 3 — the learn loop.
//
// The packet's acceptance test is a fixture: "baseline + one new silent-failure pattern -> exactly one
// learn row/work item, second pass does not duplicate". It is asserted here at the seam where the decision
// lives (candidates in, one item out, dedupe on the second call), and again in `probe-learn-dedupe.ts`
// against a real Postgres, because "never twice for the same pattern" is a partial unique index and only
// a database can prove an index refuses something.
// ---------------------------------------------------------------------------

function recorder(rows: QueryRow[] = []): { tx: QueryExecutor; sql: string[]; params: unknown[][] } {
  const sql: string[] = []
  const params: unknown[][] = []
  const tx: QueryExecutor = (strings, ...values) => {
    sql.push(strings.reduce((acc, part, i) => acc + part + (i < values.length ? `$${i + 1}` : ''), '').replace(/\s+/g, ' ').trim())
    params.push(values)
    return Promise.resolve(rows)
  }
  return { tx, sql, params }
}

const candidate = (over: Partial<LearnCandidate> = {}): LearnCandidate => ({
  pattern: 'empty-catch',
  key: 'empty-catch:app/x/route.ts',
  severity: 'normal',
  title: 'empty-catch in app/x/route.ts',
  evidence: ['app/x/route.ts:12'],
  hitCount: 1,
  firstSeen: '2026-09-15T00:00:00.000Z',
  lastSeen: '2026-09-15T01:00:00.000Z',
  ...over,
})

// --- the acceptance fixture --------------------------------------------------

test('baseline + one new pattern files exactly one item; the second pass does not duplicate', () => {
  const candidates = [candidate()]
  const first = decideLearnItem({ candidates, openPatternKeys: new Set() })
  assert.ok(first.filed, 'one new pattern must be filed')
  assert.deepEqual(first.deferred, [], 'nothing else was waiting')
  assert.deepEqual(first.skipped, [])

  // Second pass: the same evidence, with the pattern now open. This is the packet's de-dupe line.
  const second = decideLearnItem({ candidates, openPatternKeys: new Set([first.filed.key]) })
  assert.equal(second.filed, null)
  assert.deepEqual(second.skipped.map((item) => item.key), ['empty-catch:app/x/route.ts'])
})

test('the cap is one per pass, and P0 wins the slot', () => {
  assert.equal(MAX_LEARN_ITEMS_PER_PASS, 1)
  const decision = decideLearnItem({
    candidates: [
      candidate({ key: 'empty-catch:a.ts', hitCount: 9 }),
      candidate({ pattern: 'stale-claim', key: 'stale-claim', severity: 'P0', hitCount: 1, title: '1 abandoned claim' }),
      candidate({ key: 'swallowed-catch:b.ts', hitCount: 3 }),
    ],
    openPatternKeys: new Set(),
  })
  assert.equal(decision.filed?.key, 'stale-claim', 'an abandoned claim outranks code findings')
  assert.deepEqual(decision.deferred.map((item) => item.key), ['empty-catch:a.ts', 'swallowed-catch:b.ts'])
})

test('with no P0 in play the most-hit pattern goes first, then alphabetical', () => {
  const decision = decideLearnItem({
    candidates: [
      candidate({ key: 'b-second.ts', hitCount: 2 }),
      candidate({ key: 'a-first.ts', hitCount: 5 }),
    ],
    openPatternKeys: new Set(),
  })
  assert.equal(decision.filed?.key, 'a-first.ts')

  const tied = decideLearnItem({
    candidates: [candidate({ key: 'z.ts', hitCount: 1 }), candidate({ key: 'a.ts', hitCount: 1 })],
    openPatternKeys: new Set(),
  })
  assert.equal(tied.filed?.key, 'a.ts', 'a tie is broken deterministically, so two workers agree')
})

// --- candidates from real content --------------------------------------------

test('a new silent-failure pattern becomes a candidate keyed per pattern+path', () => {
  const candidates = learnCandidatesFromFiles([
    {
      path: 'app/api/widgets/route.ts',
      content: 'export async function GET() {\n  return fetchRemote().catch(() => [])\n}\n',
      firstSeen: '2026-09-15T00:00:00.000Z',
      lastSeen: '2026-09-15T02:00:00.000Z',
    },
  ])
  assert.equal(candidates.length, 1)
  assert.equal(candidates[0].pattern, 'swallowed-catch')
  assert.equal(candidates[0].key, 'swallowed-catch:app/api/widgets/route.ts')
  assert.equal(candidates[0].severity, 'normal')
  assert.match(candidates[0].evidence[0], /^app\/api\/widgets\/route\.ts:\d+$/)
  assert.equal(candidates[0].lastSeen, '2026-09-15T02:00:00.000Z')
})

test('a stale claim is one P0 candidate carrying the work item ids as evidence', () => {
  const stale = staleClaimCandidate(
    [
      { id: 'wi-1', updatedAt: '2026-09-15T00:10:00.000Z' },
      { id: 'wi-2', updatedAt: '2026-09-15T00:20:00.000Z' },
    ],
    '2026-09-15T03:00:00.000Z',
  )
  assert.ok(stale)
  assert.equal(stale.key, 'stale-claim')
  assert.equal(stale.severity, 'P0')
  assert.equal(stale.hitCount, 2)
  assert.deepEqual(stale.evidence, ['agent_work_item:wi-1', 'agent_work_item:wi-2'])
  assert.equal(staleClaimCandidate([], '2026-09-15T03:00:00.000Z'), null)
})

// --- keys, ids and the window ------------------------------------------------

test('pattern keys are per-file for code findings and whole-system for a stale claim', () => {
  assert.equal(learnPatternKey('empty-catch', 'app/x.ts'), 'empty-catch:app/x.ts')
  assert.equal(learnPatternKey('stale-claim', null), 'stale-claim')
  assert.equal(learnPatternKey('empty-catch', '/app/x.ts'), 'empty-catch:app/x.ts')
  assert.equal(learnStoryId('empty-catch:app/api/x/route.ts', '2026-09-15T04:00:00.000Z'), 'LEARN-EMPTY-CATCH-APP-API-X-ROUTE-TS-20260915')
})

test('the window is the anchor, capped at 24 hours', () => {
  const now = new Date('2026-09-15T12:00:00.000Z')
  const recent = learnWindowStart({ at: '2026-09-15T11:57:00.000Z', lastKey: null }, now)
  assert.equal(recent.toISOString(), '2026-09-15T11:57:00.000Z', 'the anchor wins when it is inside the cap')

  const ancient = learnWindowStart({ at: '2026-09-01T00:00:00.000Z', lastKey: null }, now)
  assert.equal(ancient.toISOString(), '2026-09-14T12:00:00.000Z', 'an old anchor cannot widen the window')
  assert.equal(DEFAULT_LEARN_WINDOW_HOURS, 24)

  const none = learnWindowStart(null, now)
  assert.equal(none.toISOString(), '2026-09-14T12:00:00.000Z')
  assert.equal(parseLearnAnchor('{ not json')?.at, undefined)
  assert.equal(parseLearnAnchor('{"at":"2026-09-15T00:00:00.000Z","lastKey":"x"}')?.lastKey, 'x')
})

// --- the packet the loop generates -------------------------------------------

test('the generated packet is a packet the lint would accept, and it makes no scope decision', () => {
  const packet = renderLearnPacket(candidate(), {
    storyId: 'LEARN-EMPTY-CATCH-APP-X-ROUTE-TS-20260915',
    windowStart: '2026-09-15T00:00:00.000Z',
    windowEnd: '2026-09-15T01:00:00.000Z',
  })
  assert.match(packet, /^# LEARN-EMPTY-CATCH-APP-X-ROUTE-TS-20260915 — learn: empty-catch:app\/x\/route\.ts/)
  assert.match(packet, /## Skills\n\nworkflow\n/)
  assert.match(packet, /intent: grow/)
  assert.match(packet, /## Test mode\n\nSCOPED/)
  assert.match(packet, /app\/x\/route\.ts:12/)
  // The loop states who decides, and never tells a lane to commit.
  assert.match(packet, /Lead and Architect/)
  assert.ok(
    !/\b(scout|assay|inspector)\b[^.\n]{0,60}\bcommit/i.test(packet),
    'a generated packet must not instruct a non-builder role to commit',
  )
  assert.ok(!/'status':\s*'active'|promote a decision/i.test(packet), 'the loop never promotes decisions')
})

test('the instructions with the filed item say who decides and what never happens', () => {
  const instructions = buildLearnItemInstructions(candidate({ hitCount: 3 }))
  assert.match(instructions, /Lead and Architect decide/)
  assert.match(instructions, /Assay does not ship code/)
  assert.match(instructions, /Never auto-merge/)
  assert.match(instructions, /3 hit\(s\)/)
})

// --- filing, through the repository seam -------------------------------------

test('a staged learn member carries kind=learn and its pattern key', async () => {
  const batch = recorder([{ id: 'batch-1', label: 'staging', status: 'Staged', story_count: 0, queued_count: 0, skipped_count: 0, model_policy: 'cheap', created_at: '2026-09-15T00:00:00.000Z' }])
  // ensureStagingBatch asks for the open batch first; the same fake answers both questions.
  const staged = await stageLearnStory({ storyId: 'LEARN-X', patternKey: 'stale-claim' }, batch.tx)
  assert.equal(staged.staged, true)
  const insert = batch.sql.find((sql) => sql.includes('insert into forge_batch_item')) ?? ''
  assert.match(insert, /kind, learn_pattern_key/)
  assert.deepEqual(batch.params.find((values) => values.includes('stale-claim')), ['batch-1', 'LEARN-X', 'stale-claim'])
})

test('a P0 learn item opens Ready and is stamped with kind, pattern key and instructions', async () => {
  const calls = recorder([{ id: 'wi-1' }])
  const stamped = await openReadyLearnItem(
    { storyId: 'LEARN-X', patternKey: 'stale-claim', instructions: 'Lead and Architect decide SMITH or HOLD.' },
    calls.tx,
  )
  assert.equal(stamped, 1)
  const statusCall = calls.sql.find((sql) => sql.includes('update storyboard_story')) ?? ''
  const stampCall = calls.sql.find((sql) => sql.includes('update agent_work_item')) ?? ''
  assert.match(statusCall, /set status = \$1/)
  assert.match(stampCall, /kind = 'learn'/)
  assert.match(stampCall, /learn_pattern_key = \$1/)
  assert.deepEqual(calls.params[calls.params.length - 1], ['stale-claim', 'Lead and Architect decide SMITH or HOLD.', 'LEARN-X'])
})

test('the open-pattern read looks at BOTH open work items and staged members', async () => {
  const { tx, sql } = recorder([{ key: 'stale-claim' }])
  const keys = await listOpenLearnPatternKeys(tx)
  assert.deepEqual(keys, ['stale-claim'])
  assert.match(sql[0], /from agent_work_item/)
  assert.match(sql[0], /state in \('Ready', 'Claimed', 'Running', 'Paused'\)/)
  assert.match(sql[0], /union/)
  assert.match(sql[0], /from forge_batch_item/)
  assert.match(sql[0], /state = 'Staged'/)
})

// --- what the loop refuses to learn from -------------------------------------

test('the loop does not learn from its own fixtures or the detector itself', () => {
  // Measured on the first dry run: 9 candidates, 5 of them from these two classes. A test that asserts
  // "this IS a swallowed catch" contains a swallowed catch, and the hunter's own source describes all four
  // patterns. Filing either is how a learn loop becomes noise everyone mutes.
  assert.equal(isLearnWindowPath('agent-runtime/silent-failure-patterns.test.ts'), false)
  assert.equal(isLearnWindowPath('agent-runtime/silent-failure-patterns.ts'), false)
  assert.equal(isLearnWindowPath('app/portal/tech/actions.test.tsx'), false)
  assert.equal(isLearnWindowPath('lib/thing.spec.ts'), false)
  // Real surface still counts.
  assert.equal(isLearnWindowPath('app/portal/tech/actions.ts'), true)
  assert.equal(isLearnWindowPath('legacy/services/property/repository.ts'), true)
  // Docs, generated output and the frozen engine tree are not surfaces.
  assert.equal(isLearnWindowPath('docs/agent/MEMORY.md'), false)
  assert.equal(isLearnWindowPath('testv2/engine_tests/hardening.test.ts'), false)
  assert.equal(isLearnWindowPath('.next/types/route.ts'), false)
})
