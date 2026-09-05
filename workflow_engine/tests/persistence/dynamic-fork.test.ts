import { test } from 'node:test'
import assert from 'node:assert/strict'

import { assertEngineSchema, PersistenceFixture } from './harness'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — engine <dynamic-fork> synchronous fan-out primitive (real DB).
//
// A dynamic-fork reads the branch count from its count-variable process
// variable, clamps to minimum..maximum, completes the parent token, and fans
// N child tokens to the join node. The existing join (fork-parent correlation)
// releases exactly once when all N arrive, so the process continues to the
// successor. Run via the persistence harness (test:persistence).
// ---------------------------------------------------------------------------

const DYNAMIC_FORK_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'split' }] },
    split: {
      id: 'split',
      type: 'dynamic-fork',
      countVariable: 'splitCount',
      branchCommandType: 'forge.run_smith_split',
      join: 'j',
      minimum: 2,
      maximum: 8,
    },
    j: { id: 'j', type: 'join', transitions: [{ name: 'complete', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

test('ENG-FORGE-V9: a dynamic-fork fans out N branch tokens and rejoins once', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_dynfork', 1, DYNAMIC_FORK_GRAPH as any)
    const engine = f.makeEngine()
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_dynfork',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
      variables: { splitCount: 3 },
    })

    const inst = await f.instance(processInstanceId)
    assert.ok(inst, 'instance row persisted')
    assert.equal(inst.status, 'completed', 'process must reach completion through the split')

    const tokens = (await f.tokens(processInstanceId)) as any[]
    // parent (split) + 3 join children + 1 end successor.
    assert.equal(tokens.length, 5, `expected 5 tokens, got ${tokens.length}`)
    const joinTokens = tokens.filter((t) => t.node_id === 'j')
    assert.equal(joinTokens.length, 3, 'exactly 3 fork branch tokens must reach the join')
    const endToken = tokens.filter((t) => t.node_id === 'end')
    assert.equal(endToken.length, 1, 'the join must release exactly one successor to end')

    const events = (await f.events(processInstanceId)) as any[]
    const forked = events.filter((e) => e.event_type === 'token.forked')
    assert.equal(forked.length, 3, 'exactly 3 token.forked events')
  } finally {
    await f.cleanup()
  }
})

test('ENG-FORGE-V9: dynamic-fork count is clamped to maximum', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_dynfork_clamp', 1, DYNAMIC_FORK_GRAPH as any)
    const engine = f.makeEngine()
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_dynfork_clamp',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
      variables: { splitCount: 99 }, // exceeds maximum 8 -> clamped to 8
    })

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
    const tokens = (await f.tokens(processInstanceId)) as any[]
    const joinTokens = tokens.filter((t) => t.node_id === 'j')
    assert.equal(joinTokens.length, 8, 'count must clamp to the configured maximum (8)')
  } finally {
    await f.cleanup()
  }
})
