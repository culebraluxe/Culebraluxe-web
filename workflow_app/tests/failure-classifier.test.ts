import { test } from 'node:test'
import assert from 'node:assert/strict'
import { classifyFailure, routeFailure, isForgeFailureClass } from '../forge/failure-classifier'

// ENG-FORGE-HARDEN-02 — canonical failure classification + deterministic,
// bounded routing. FAIL -> CLASSIFY -> ROUTE; UNKNOWN fails safely to HOLD.

test('classification: an explicit canonical class is preserved', () => {
  const c = classifyFailure({ candidate: 'BAD_IMPLEMENTATION', detail: 'tests red on candidate' })
  assert.equal(c.class, 'BAD_IMPLEMENTATION')
  assert.equal(c.unknown, false)
})

test('classification: narrow unambiguous signals infer a canonical class', () => {
  assert.equal(classifyFailure({ observed: 'deploy' }).class, 'DEPLOYMENT_FAILURE')
  assert.equal(classifyFailure({ observed: 'env' }).class, 'ENVIRONMENT_FAILURE')
  assert.equal(classifyFailure({ observed: 'missing-context' }).class, 'MISSING_CONTEXT')
})

test('classification: an invalid/free-text candidate resolves to UNKNOWN (safe)', () => {
  const c = classifyFailure({ candidate: 'it just broke somehow' })
  assert.equal(c.class, 'UNKNOWN')
  assert.equal(c.unknown, true)
  assert.ok(!isForgeFailureClass('it just broke somehow'))
})

test('BAD_IMPLEMENTATION -> Smith repair', () => {
  const r = routeFailure({ class: 'BAD_IMPLEMENTATION', attempts: 1, maxAttempts: 3 })
  assert.deepEqual(r, { action: 'repair', owner: 'smith', attempts: 1 })
})

test('MISSING_CONTEXT -> Scout (escapes the Smith loop)', () => {
  const r = routeFailure({ class: 'MISSING_CONTEXT', attempts: 1, maxAttempts: 3 })
  assert.deepEqual(r, { action: 'repair', owner: 'scout', attempts: 1 })
})

test('BAD_ARCHITECTURE -> Architect', () => {
  const r = routeFailure({ class: 'BAD_ARCHITECTURE', attempts: 1, maxAttempts: 3 })
  assert.deepEqual(r, { action: 'repair', owner: 'architect', attempts: 1 })
})

test('DEPLOYMENT_FAILURE and ENVIRONMENT_FAILURE -> DEV_OPS', () => {
  const d = routeFailure({ class: 'DEPLOYMENT_FAILURE', attempts: 1, maxAttempts: 3 })
  const e = routeFailure({ class: 'ENVIRONMENT_FAILURE', attempts: 1, maxAttempts: 3 })
  assert.equal(d.action, 'repair')
  assert.equal(d.action === 'repair' ? d.owner : '', 'dev_ops')
  assert.equal(e.action, 'repair')
  assert.equal(e.action === 'repair' ? e.owner : '', 'dev_ops')
})

test('UNKNOWN and DEPENDENCY_FAILURE fail safely to HOLD', () => {
  assert.equal(routeFailure({ class: 'UNKNOWN', attempts: 1, maxAttempts: 3 }).action, 'hold')
  assert.equal(routeFailure({ class: 'DEPENDENCY_FAILURE', attempts: 1, maxAttempts: 3 }).action, 'hold')
})

test('retry exhaustion HOLDs instead of authorizing another cycle', () => {
  const r = routeFailure({ class: 'BAD_IMPLEMENTATION', attempts: 3, maxAttempts: 3 })
  assert.equal(r.action, 'hold')
  assert.match(r.action === 'hold' ? r.reason : '', /exhausted/)
})

test('other classes route to their documented owners', () => {
  const w = routeFailure({ class: 'WEAK_TEST', attempts: 1, maxAttempts: 3 })
  const t = routeFailure({ class: 'BAD_TOOL_CONTRACT', attempts: 1, maxAttempts: 3 })
  const g = routeFailure({ class: 'MISSING_GUARDRAIL', attempts: 1, maxAttempts: 3 })
  assert.equal(w.action === 'repair' ? w.owner : '', 'qa')
  assert.equal(t.action === 'repair' ? t.owner : '', 'lead')
  assert.equal(g.action === 'repair' ? g.owner : '', 'lead')
})
