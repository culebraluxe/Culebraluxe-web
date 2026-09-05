import assert from 'node:assert/strict'
import test from 'node:test'

import { decideForgeTransition, type ForgeTransitionDecision } from './forge-transition'
import {
  assertForgeDecisionOnTopology,
  ensureForgeSdlcTopology,
  forgeDecisionNodeId,
  forgeSdlcTopology,
  resetForgeSdlcTopologyCache,
  type ForgeSdlcTopology,
} from './forge-topology'

// ---------------------------------------------------------------------------
// ENG-FORGE-V8 — FORGE_SDLC as the single topology contract of the live driver.
//
// Proves forge-transition.ts (the live Forge reducer) maps onto the canonical
// FORGE_SDLC-v1.xml topology: every reducer outcome resolves to a real XML node
// with the expected type/responsibility, the termini are exactly
// story_complete + forge_hold, and the guard fails closed on drift.
//
// No database, no packages. The topology loads through the shared four-layer
// pipeline (parseForgeSdlc) exactly as V7 established.
// ---------------------------------------------------------------------------

const CANDIDATE = 'a'.repeat(40)
const HOLD_EVENTS = [
  { type: 'lead-pre', decision: 'HOLD', detail: 'veto' },
  { type: 'lead-pre', decision: 'SPLIT', splitCount: 3 },
  { type: 'smith-failed', code: 'SMITH_RESULT_FAILED', detail: 'failed' },
  { type: 'assay-fail', code: 'ASSAY_TEST_FAILED', detail: '1 failed' },
  { type: 'assay-runtime-interrupted', detail: 'interrupted' },
  { type: 'publish-conflict', detail: 'main advanced' },
] as const

/** Every distinct action the reducer can emit, driven through real events. */
function reducerDecisions(): ForgeTransitionDecision[] {
  return [
    decideForgeTransition({ type: 'architect-complete' }),
    decideForgeTransition({ type: 'lead-pre', decision: 'SOLO' }),
    decideForgeTransition({ type: 'lead-pre', decision: 'SMITH' }),
    decideForgeTransition({ type: 'lead-implement-complete', candidateSha: CANDIDATE }),
    decideForgeTransition({ type: 'smith-complete', candidateSha: CANDIDATE }),
    decideForgeTransition({ type: 'lead-post', decision: 'ASSAY', candidateSha: CANDIDATE }),
    decideForgeTransition({
      type: 'smith-runtime-interrupted',
      attempts: 1,
      maxAttempts: 3,
      detail: 'transient',
    }),
    decideForgeTransition({ type: 'assay-pass' }),
    decideForgeTransition({ type: 'publish-complete' }),
    ...HOLD_EVENTS.map((event) => decideForgeTransition(event)),
  ]
}

test('ENG-FORGE-V8: FORGE_SDLC loads as the live topology (17 nodes, two termini)', () => {
  resetForgeSdlcTopologyCache()
  const topology = forgeSdlcTopology()
  assert.equal(topology.key, 'FORGE_SDLC')
  assert.equal(topology.version, 1)
  assert.equal(topology.nodeIds.size, 17)
  assert.deepEqual([...topology.endIds].sort(), ['forge_hold', 'story_complete'])
})

test('ENG-FORGE-V8: serial backbone Ready -> Scout -> Architect -> Lead PRE is present', () => {
  const topology = forgeSdlcTopology()
  for (const id of ['ready', 'scout', 'architect', 'lead_pre']) {
    assert.ok(topology.nodeIds.has(id), `backbone node '${id}' must exist`)
  }
})

test('ENG-FORGE-V8: every FORGE_SDLC task carries a Forge position responsibility', () => {
  const topology = forgeSdlcTopology()
  const valid = new Set(['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'])
  const tasks = Object.values(topology.tasks)
  assert.ok(tasks.length > 0, 'FORGE_SDLC must define task nodes')
  for (const task of tasks) {
    assert.ok(
      task.responsibility !== undefined && valid.has(task.responsibility),
      `task '${task.id}' responsibility '${task.responsibility ?? '(none)'}' must be a Forge position`,
    )
  }
})

test('ENG-FORGE-V8: every reducer decision maps to a real FORGE_SDLC node', () => {
  const topology = forgeSdlcTopology()
  for (const decision of reducerDecisions()) {
    const nodeId = forgeDecisionNodeId(decision)
    assert.ok(
      topology.nodeIds.has(nodeId),
      `action='${decision.action}' must map to a FORGE_SDLC node`,
    )
    assert.doesNotThrow(() => assertForgeDecisionOnTopology(decision, topology))
  }
})

test('ENG-FORGE-V8: the full V6 action set is covered with no orphan outcome', () => {
  const topology = forgeSdlcTopology()
  const actions = new Set(reducerDecisions().map((d) => d.action))
  assert.deepEqual(
    [...actions].sort(),
    [
      'complete',
      'enqueue-assay',
      'enqueue-lead',
      'enqueue-smith',
      'hold-human',
      'publish',
      'retry-same-lane',
    ].sort(),
  )
  for (const decision of reducerDecisions()) {
    assert.doesNotThrow(() => forgeDecisionNodeId(decision))
  }
  assert.ok(assertForgeDecisionOnTopology, 'guard is exported')
})

test('ENG-FORGE-V8: Lead phases resolve to the Lead task nodes with lead responsibility', () => {
  const topology = forgeSdlcTopology()
  const leadTasks = ['lead_pre', 'lead_implement', 'lead_post']
  for (const id of leadTasks) {
    assert.equal(topology.tasks[id]?.responsibility, 'lead', `'${id}' must be a lead task`)
  }
  assert.equal(
    forgeDecisionNodeId(decideForgeTransition({ type: 'architect-complete' })),
    'lead_pre',
  )
  assert.equal(
    forgeDecisionNodeId(decideForgeTransition({ type: 'lead-pre', decision: 'SOLO' })),
    'lead_implement',
  )
  assert.equal(
    forgeDecisionNodeId(decideForgeTransition({ type: 'smith-complete', candidateSha: CANDIDATE })),
    'lead_post',
  )
})

test('ENG-FORGE-V8: hold-human parks on forge_hold; complete terminates on story_complete', () => {
  const topology = forgeSdlcTopology()
  for (const event of HOLD_EVENTS) {
    const decision = decideForgeTransition(event)
    assert.equal(decision.action, 'hold-human')
    assert.equal(forgeDecisionNodeId(decision), 'forge_hold')
    assert.ok(topology.endIds.has('forge_hold'))
  }
  const done = decideForgeTransition({ type: 'publish-complete' })
  assert.equal(done.action, 'complete')
  assert.equal(forgeDecisionNodeId(done), 'story_complete')
  assert.ok(topology.endIds.has('story_complete'))
})

test('ENG-FORGE-V8: Assay and publish map to QA / DEV_OPS responsibilities', () => {
  const topology = forgeSdlcTopology()
  assert.equal(forgeDecisionNodeId(decideForgeTransition({ type: 'assay-pass' })), 'publish')
  assert.equal(topology.tasks.publish?.responsibility, 'dev_ops')
  const assay = decideForgeTransition({
    type: 'lead-post',
    decision: 'ASSAY',
    candidateSha: CANDIDATE,
  })
  assert.equal(forgeDecisionNodeId(assay), 'assay')
  assert.equal(topology.tasks.assay?.responsibility, 'qa')
})

test('ENG-FORGE-V8: retry-same-lane stays on the Smith task node', () => {
  const decision = decideForgeTransition({
    type: 'smith-runtime-interrupted',
    attempts: 1,
    maxAttempts: 3,
    detail: 'transient',
  })
  assert.equal(decision.action, 'retry-same-lane')
  assert.equal(forgeDecisionNodeId(decision), 'smith')
})

test('ENG-FORGE-V8: unknown reducer action fails the guard closed (no silent drift)', () => {
  resetForgeSdlcTopologyCache()
  const topology = forgeSdlcTopology()
  const bogus = {
    action: 'bogus-action',
    nextLane: null,
    nextPhase: null,
    storyStatus: null,
    humanRequired: false,
    failure: null,
  } as unknown as ForgeTransitionDecision
  assert.throws(() => forgeDecisionNodeId(bogus), /no FORGE_SDLC node/)
  assert.throws(() => assertForgeDecisionOnTopology(bogus, topology), /no FORGE_SDLC node/)
})

test('ENG-FORGE-V8: ensureForgeSdlcTopology returns a valid cached topology (live guard)', () => {
  resetForgeSdlcTopologyCache()
  const a = ensureForgeSdlcTopology()
  const b = ensureForgeSdlcTopology()
  assert.equal(a, b, 'guard result must be cached per process')
  assert.equal(a.key, 'FORGE_SDLC')
})

test('ENG-FORGE-V8: assertForgeDecisionOnTopology passes a real valid topology', () => {
  const topology: ForgeSdlcTopology = forgeSdlcTopology()
  for (const decision of reducerDecisions()) {
    assert.doesNotThrow(() => assertForgeDecisionOnTopology(decision, topology))
  }
})

