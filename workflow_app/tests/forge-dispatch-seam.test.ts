import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { SmithChunk, SmithExecutionPlan } from '../forge/forge-execution-shaping'
import { assessSmithWork } from '../forge/forge-dispatch-seam'

const SMALL_LINE = 'SMITH_PLAN: {"size":"SMALL","chunks":1,"proofs":["pnpm exec tsx --test a.test.ts"]}'
const MEDIUM_LINE = 'SMITH_PLAN: {"size":"MEDIUM","chunks":2,"proofs":["pnpm exec tsx --test a.test.ts","pnpm exec tsx --test b.test.ts"]}'
const OVERSIZED_LINE = 'SMITH_PLAN: {"size":"OVERSIZED","chunks":4,"proofs":[]}'
const FOUR_CHUNK_LINE = 'SMITH_PLAN: {"size":"MEDIUM","chunks":4,"proofs":[]}'
const MALFORMED_LINE = 'SMITH_PLAN: {not json'

const chunk = (id: number, over: Partial<SmithChunk> = {}): SmithChunk => ({
  id,
  outcome: `outcome ${id}`,
  surface: [`path${id}/a.ts`],
  invariant: `invariant ${id}`,
  proof: `pnpm exec tsx --test path${id}/a.test.ts`,
  ...over,
})

const plan = (chunks: SmithChunk[], size: SmithExecutionPlan['size'] = 'MEDIUM'): SmithExecutionPlan => ({
  size,
  chunks,
})

test('dispatch seam: no SMITH_PLAN line dispatches (GO) — no fabricated hold', () => {
  const a = assessSmithWork('agent finished, no plan line')
  assert.equal(a.verdict, 'GO')
  assert.equal(a.gate, 'envelope-guard')
  assert.deepEqual(a.reasons, [])
  assert.equal(a.envelope, null)
})

test('dispatch seam: in-bounds plans dispatch (GO) - preserves the current pass path', () => {
  for (const line of [SMALL_LINE, MEDIUM_LINE]) {
    const a = assessSmithWork(line)
    assert.equal(a.verdict, 'GO', `expected GO for ${line}`)
    assert.equal(a.gate, 'envelope-guard')
    assert.deepEqual(a.reasons, [])
  }
})

test('dispatch seam: OVERSIZED self-size HOLDs with the anti-token-fire reason', () => {
  const a = assessSmithWork(OVERSIZED_LINE)
  assert.equal(a.verdict, 'HOLD')
  assert.ok(a.reasons.join(' ').includes('>3 chunks is a HOLD'))
})

test('dispatch seam: a 4th chunk (MEDIUM, 4 chunks) HOLDs — not "keep working"', () => {
  const a = assessSmithWork(FOUR_CHUNK_LINE)
  assert.equal(a.verdict, 'HOLD')
})

test('dispatch seam: a malformed plan line is treated as no evidence — dispatches (GO)', () => {
  const a = assessSmithWork(MALFORMED_LINE)
  assert.equal(a.verdict, 'GO')
  assert.equal(a.envelope, null)
})

test('dispatch seam: a full structured plan runs the complete KRAKEN gate', () => {
  const sound = assessSmithWork(null, plan([chunk(1), chunk(2, { dependsOn: [1] })]))
  assert.equal(sound.gate, 'full-dispatch')
  assert.equal(sound.verdict, 'GO')
  assert.equal(sound.full?.verdict, 'GO')

  const oversized = assessSmithWork(null, plan([chunk(1), chunk(2), chunk(3), chunk(4)]))
  assert.equal(oversized.gate, 'full-dispatch')
  assert.equal(oversized.verdict, 'HOLD')
  assert.ok(oversized.reasons.join(' ').includes('4th chunk is HOLD'))
})
