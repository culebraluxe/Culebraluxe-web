import { test } from 'node:test'
import assert from 'node:assert/strict'
import { planAssay } from './assay-plan'

test('empty packet rejects instead of inventing pnpm test', () => {
  const plan = planAssay({})
  assert.equal(plan.ok, false)
  if (!plan.ok) assert.equal(plan.code, 'missing-assay-plan')
})

test('SCOPED packet with targeted node:test is accepted', () => {
  const plan = planAssay({
    testMode: 'SCOPED',
    assayCommands: '- node --test agent-runtime/assay-plan.test.ts',
  })
  assert.equal(plan.ok, true)
  if (plan.ok) {
    assert.equal(plan.mode, 'SCOPED')
    assert.equal(plan.commands.length, 1)
    assert.match(plan.instructions, /runtime test-mode: SCOPED/)
  }
})

test('pnpm test under SCOPED is rejected', () => {
  const plan = planAssay({
    testMode: 'SCOPED',
    assayCommands: 'pnpm test',
  })
  assert.equal(plan.ok, false)
  if (!plan.ok) assert.equal(plan.code, 'full-not-authorized')
})

test('FULL authorizes pnpm test', () => {
  const plan = planAssay({
    testMode: 'FULL',
    assayCommands: 'pnpm test',
  })
  assert.equal(plan.ok, true)
})
