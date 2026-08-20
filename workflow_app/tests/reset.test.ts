import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  assertDevResetAllowed,
  resetDevWorkflowsCore,
  RESET_STEPS,
  type RawSqlExecutor,
} from '../reset'

test('reset is allowed only in development', () => {
  assert.doesNotThrow(() => assertDevResetAllowed('development'))
  assert.throws(() => assertDevResetAllowed('production'), /DEV-only/)
  assert.throws(() => assertDevResetAllowed(undefined), /DEV-only/)
})

test('reset deletes children before parents and preserves definitions', async () => {
  const executed: string[] = []
  const exec: RawSqlExecutor = async (sqlText) => {
    executed.push(sqlText)
    return [{ id: 'x' }] // one row per delete
  }

  const results = await resetDevWorkflowsCore(exec)

  // Every step ran exactly once, in order.
  assert.equal(executed.length, RESET_STEPS.length)
  RESET_STEPS.forEach((step, i) => {
    assert.equal(executed[i], step.statement)
  })

  // Results carry the label and a deleted count.
  assert.deepEqual(results.map((r) => r.table), RESET_STEPS.map((s) => s.table))
  for (const r of results) assert.equal(r.deleted, 1)

  // Children (correlation, events, commands, jobs, tasks, tokens) are deleted
  // before process_instances; process_definitions is never touched.
  const piIndex = executed.findIndex((s) => s.startsWith('delete from process_instances'))
  for (const child of [
    'delete from workflow_task_correlation',
    'delete from process_events',
    'delete from process_commands',
    'delete from jobs',
    'delete from tasks',
    'delete from tokens',
  ]) {
    const idx = executed.findIndex((s) => s.startsWith(child))
    assert.ok(idx >= 0 && idx < piIndex, `${child} must precede process_instances`)
  }
  assert.equal(executed.some((s) => s.startsWith('delete from process_definitions')), false)
})
