import assert from 'node:assert/strict'
import { test } from 'node:test'
import { assessLeadPreDispatch, parseLeadPlan } from '../forge/forge-lead-plan'

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
