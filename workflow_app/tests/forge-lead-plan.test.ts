import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  assessLeadHandoff,
  assessLeadPreDispatch,
  findLatestLeadPlan,
  leadPreDispatchHoldReasons,
  parseLeadPlan,
  renderSmithWorkOrders,
} from '../forge/forge-lead-plan'

const VALID_2 = 'LEAD_PLAN: {"size":"MEDIUM","chunks":[{"id":1,"outcome":"add mapper","surface":["a/m.ts"],"invariant":"m maps","proof":"pnpm exec tsx --test a/m.test.ts"},{"id":2,"outcome":"wire route","surface":["a/r.ts"],"invariant":"r serves","proof":"pnpm exec tsx --test a/r.test.ts","dependsOn":[1]}]}'
const VALID_3 = 'LEAD_PLAN: {"size":"LARGE","chunks":[{"id":1,"outcome":"c1","surface":["f1.ts"],"invariant":"i1","proof":"p1"},{"id":2,"outcome":"c2","surface":["f2.ts"],"invariant":"i2","proof":"p2"},{"id":3,"outcome":"c3","surface":["f3.ts"],"invariant":"i3","proof":"p3"}]}'
const FOUR_CHUNK = 'LEAD_PLAN: {"size":"LARGE","chunks":[{"id":1,"outcome":"c1","surface":["f1.ts"],"invariant":"i1","proof":"p1"},{"id":2,"outcome":"c2","surface":["f2.ts"],"invariant":"i2","proof":"p2"},{"id":3,"outcome":"c3","surface":["f3.ts"],"invariant":"i3","proof":"p3"},{"id":4,"outcome":"c4","surface":["f4.ts"],"invariant":"i4","proof":"p4"}]}'
const BAD_SURFACE = 'LEAD_PLAN: {"size":"MEDIUM","chunks":[{"id":1,"outcome":"c1","surface":[],"invariant":"i1","proof":"p1"}]}'
const NO_PROOF = 'LEAD_PLAN: {"size":"SMALL","chunks":[{"id":1,"outcome":"c1","surface":["f.ts"],"invariant":"i1"}]}'
const MALFORMED = 'LEAD_PLAN: {not json'
const WRONG_SIZE = 'LEAD_PLAN: {"size":"OVERSIZED","chunks":[{"id":1,"outcome":"c1","surface":["f.ts"],"invariant":"i1","proof":"p1"}]}'

test('lead plan: parses a valid structured plan into full SmithExecutionPlan chunks', () => {
  const p = parseLeadPlan(`some prose\n${VALID_2}\nmore`)
  assert.ok(p)
  assert.equal(p!.size, 'MEDIUM')
  assert.equal(p!.chunks.length, 2)
  assert.deepEqual(p!.chunks[1].surface, ['a/r.ts'])
  assert.deepEqual(p!.chunks[1].dependsOn, [1])
})

test('lead plan: absent or non-lead notes return null (never fabricate)', () => {
  assert.equal(parseLeadPlan('no plan here'), null)
  assert.equal(parseLeadPlan(null), null)
  assert.equal(parseLeadPlan('SMITH_PLAN: {"size":"SMALL","chunks":1}'), null, 'a Smith envelope is not a Lead plan')
})

test('lead plan: malformed / incomplete / invalid-size plans return null', () => {
  assert.equal(parseLeadPlan(MALFORMED), null)
  assert.equal(parseLeadPlan(BAD_SURFACE), null)
  assert.equal(parseLeadPlan(NO_PROOF), null)
  assert.equal(parseLeadPlan(WRONG_SIZE), null)
})

test('lead plan: parses the work-order vocabulary (scope/acceptance/postconditions)', () => {
  const wo =
    'LEAD_PLAN: {"size":"SMALL","chunks":[{"id":1,"scope":["workflow_app/forge/forge-ready-gate.ts#storyReadyToRunReasons"],"acceptance":"pnpm exec tsx --test workflow_app/tests/forge-ready-gate.test.ts","preconditions":[],"postconditions":"zero-command recipe -> missing-assay-plan"}]}'
  const p = parseLeadPlan(wo)
  assert.ok(p, 'work-order vocabulary plan must parse')
  assert.equal(p!.size, 'SMALL')
  assert.equal(p!.chunks.length, 1)
  assert.deepEqual(p!.chunks[0].surface, [
    'workflow_app/forge/forge-ready-gate.ts#storyReadyToRunReasons',
  ])
  assert.equal(p!.chunks[0].proof, 'pnpm exec tsx --test workflow_app/tests/forge-ready-gate.test.ts')
  assert.ok(p!.chunks[0].invariant.includes('zero-command'))
})

test('smith work orders: renders each chunk and finds the latest lead plan in runs', () => {
  const plan = parseLeadPlan(VALID_2)
  assert.ok(plan)
  const rendered = renderSmithWorkOrders(plan!)
  assert.match(rendered, /Chunk 1:/)
  assert.match(rendered, /Scope: a\/m.ts/)
  assert.match(rendered, /Acceptance .*: pnpm exec tsx --test a\/m.test.ts/)
  assert.match(rendered, /Chunk 2:/)
  assert.match(rendered, /Preconditions: depends on chunk 1/)
  // latest-lead-plan finder: prefers the LAST lead run carrying a LEAD_PLAN.
  const found = findLatestLeadPlan([
    { runType: 'architect', notes: 'nope' },
    { runType: 'lead', notes: 'earlier no plan' },
    { runType: 'lead', notes: `prose\n${VALID_2}` },
  ])
  assert.ok(found)
  assert.equal(found!.chunks.length, 2)
  assert.equal(findLatestLeadPlan([{ runType: 'smith', notes: VALID_2 }]), null)
  assert.equal(findLatestLeadPlan(null), null)
})

test('lead pre-dispatch: no plan is NO_PLAN, not a HOLD (behavior-preserving)', () => {
  const a = assessLeadPreDispatch('decision only, no LEAD_PLAN')
  assert.equal(a.planPresent, false)
  assert.equal(a.verdict, 'NO_PLAN')
  assert.deepEqual(a.reasons, [])
})

test('lead pre-dispatch: a sound in-bounds plan dispatches (GO) through the full gate', () => {
  const a = assessLeadPreDispatch(VALID_2)
  assert.equal(a.planPresent, true)
  assert.equal(a.verdict, 'GO')
  assert.equal(a.full?.verdict, 'GO')
})

test('lead pre-dispatch: a 4-chunk plan HOLDs (a 4th chunk is not keep working)', () => {
  const a = assessLeadPreDispatch(FOUR_CHUNK)
  assert.equal(a.planPresent, true)
  assert.equal(a.verdict, 'HOLD')
  assert.ok(a.reasons.join(' ').includes('4th chunk is HOLD'))
})

test('lead handoff gate: a HOLD verdict on a Smith dispatch actually blocks the handoff', () => {
  const holds = leadPreDispatchHoldReasons('SMITH', FOUR_CHUNK)
  assert.ok(holds.length > 0, 'SMITH dispatch with an oversized LEAD_PLAN must hold pre-Smith')
  assert.ok(holds[0]!.startsWith('lead-plan:'))
  // SPLIT dispatches gate identically.
  assert.ok(leadPreDispatchHoldReasons('SPLIT', FOUR_CHUNK).length > 0)
})

test('lead handoff gate: NO_PLAN on a SMITH/SPLIT dispatch now HOLDS (authoritative pre-Smith fuse)', () => {
  // Sound in-bounds plan on a SMITH dispatch => no hold.
  assert.deepEqual(leadPreDispatchHoldReasons('SMITH', VALID_2), [])
  // Decisions that do not dispatch (SOLO/HOLD) are not gated even with a plan present.
  assert.deepEqual(leadPreDispatchHoldReasons('SOLO', FOUR_CHUNK), [])
  assert.deepEqual(leadPreDispatchHoldReasons('HOLD', FOUR_CHUNK), [])
  assert.deepEqual(leadPreDispatchHoldReasons(undefined, FOUR_CHUNK), [])
  // A dispatch with NO assessable LEAD_PLAN must HOLD back to Lead (not start Smith).
  const noPlanSmith = leadPreDispatchHoldReasons('SMITH', 'decision only')
  assert.ok(noPlanSmith.length > 0, 'SMITH with no LEAD_PLAN must hold pre-Smith')
  assert.ok(noPlanSmith[0]!.includes('lead-plan:missing'))
  assert.ok(leadPreDispatchHoldReasons('SPLIT', 'decision only').length > 0)
})

// The review's assertion, at the level the RUNNER actually decides: a Lead that
// routes SMITH/SPLIT with no parseable LEAD_PLAN never hands off to a write lane.
test('lead handoff gate: the runner HOLDs a SMITH/SPLIT dispatch with missing or malformed LEAD_PLAN', () => {
  // Missing plan -> HOLD (no Smith adapter may start).
  const missing = assessLeadHandoff('lead_pre', 'SMITH', 'I decided to dispatch to Smith.')
  assert.equal(missing.applies, true)
  assert.equal(missing.verdict, 'HOLD')
  assert.ok(missing.holds[0]!.includes('lead-plan:missing'))

  // Malformed plan -> HOLD, and the reason names the plan problem, not just absence.
  const malformed = assessLeadHandoff('lead_pre', 'SMITH', MALFORMED)
  assert.equal(malformed.verdict, 'HOLD')
  assert.ok(malformed.holds.length > 0)

  // SPLIT gates identically.
  assert.equal(assessLeadHandoff('lead_pre', 'SPLIT', 'no plan here').verdict, 'HOLD')

  // A valid in-bounds plan hands off.
  assert.deepEqual(assessLeadHandoff('lead_pre', 'SMITH', VALID_2), {
    applies: true,
    holds: [],
    verdict: 'GO',
  })

  // Not a dispatch decision -> not gated.
  assert.equal(assessLeadHandoff('lead_pre', 'SOLO', 'no plan here').verdict, 'GO')

  // The gate is SCOPED to the Lead handoff: on a smith node it never applies, so a
  // post-Smith node cannot be held for a missing Lead plan.
  assert.deepEqual(assessLeadHandoff('smith', 'SMITH', 'no plan here'), {
    applies: false,
    holds: [],
    verdict: 'NOT_GATED',
  })
})
