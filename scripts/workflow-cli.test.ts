// ---------------------------------------------------------------------------
// ENG-07 — targeted unit tests for the thin workflow CLI dispatcher.
//
// The CLI adds no workflow logic: each subcommand maps 1:1 to an existing
// typed function. These tests pin the dispatch surface (subcommand -> seam,
// argument parsing, reset:dev confirmation, pnpm delegation) with fakes, so
// no database or engine is required.
//
// Run directly (not part of the forbidden harness globs):
//   node_modules/.bin/tsx --test scripts/workflow-cli.test.ts
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  runWorkflowCliCore,
  DEFAULT_POLL_WORKER_ID,
  DEFAULT_RECLAIM_BATCH,
  DEFAULT_POLL_BATCH,
  type WorkflowCliDeps,
} from './workflow-cli'
import type { WorkflowDiagnosticsSnapshot } from '../workflow_app/diagnostics'

const SNAPSHOT: WorkflowDiagnosticsSnapshot = {
  configured: true,
  summary: {
    definitionCount: 1,
    instanceTotal: 2,
    instanceActive: 1,
    instanceCompleted: 1,
    instanceFailed: 0,
    instanceOther: 0,
    readyEngineTasks: 0,
    correlatedOpenCanonicalTasks: 1,
    pendingJobs: 2,
    pendingReceipts: 0,
    anomalyCount: 1,
  },
  definitions: [],
  instances: [],
  anomalies: [
    {
      kind: 'stale-locked-job',
      severity: 'warning',
      instanceId: 'i-1',
      subjectId: null,
      message: 'stale lock',
    },
  ],
}

function makeDeps(trace: string[]): WorkflowCliDeps {
  return {
    status: async () => {
      trace.push('status')
      return SNAPSHOT
    },
    reconcile: async () => {
      trace.push('reconcile')
      return { startedInstances: 1, materializedTasks: 2, skippedTasks: 0 }
    },
    reclaimStaleJobs: async (batch) => {
      trace.push(`reclaim:${batch}`)
      return 3
    },
    runDueJobsPass: async (workerId, batch) => {
      trace.push(`poll:${workerId}:${batch}`)
      return { reclaimed: 1, claimed: 2, fired: 1, completed: 1, failed: 0 }
    },
    runPnpmScript: async (script) => {
      trace.push(`pnpm:${script}`)
      return script === 'test:persistence' ? 2 : 0
    },
    resetDev: async () => {
      trace.push('resetDev')
      return [{ table: 'jobs', deleted: 4 }]
    },
  }
}

test('status delegates to the read-only snapshot and reports the anomaly count', async () => {
  const trace: string[] = []
  const outcome = await runWorkflowCliCore(makeDeps(trace), ['status'])

  assert.equal(outcome.code, 0)
  assert.deepEqual(trace, ['status'])
  assert.match(outcome.text, /workflow status \(read-only snapshot\)/)
  assert.match(outcome.text, /anomalies: 1/)
  assert.match(outcome.text, /stale-locked-job/)
})

test('reconcile delegates to reconcileWorkflows', async () => {
  const trace: string[] = []
  const outcome = await runWorkflowCliCore(makeDeps(trace), ['reconcile'])

  assert.equal(outcome.code, 0)
  assert.deepEqual(trace, ['reconcile'])
  assert.match(outcome.text, /started instances: 1/)
  assert.match(outcome.text, /materialized tasks: 2/)
})

test('reclaim uses the default batch and parses an explicit batch', async () => {
  const trace: string[] = []
  const def = await runWorkflowCliCore(makeDeps(trace), ['reclaim'])
  assert.equal(def.code, 0)
  assert.deepEqual(trace, [`reclaim:${DEFAULT_RECLAIM_BATCH}`])
  assert.match(def.text, /reclaimed 3 stale job lease/)

  trace.length = 0
  const explicit = await runWorkflowCliCore(makeDeps(trace), ['reclaim', '5'])
  assert.equal(explicit.code, 0)
  assert.deepEqual(trace, ['reclaim:5'])

  trace.length = 0
  const bad = await runWorkflowCliCore(makeDeps(trace), ['reclaim', 'nope'])
  assert.equal(bad.code, 1)
  assert.deepEqual(trace, [], 'invalid batch must not reach the seam')
})

test('poll uses default worker/batch and parses explicit worker/batch', async () => {
  const trace: string[] = []
  const def = await runWorkflowCliCore(makeDeps(trace), ['poll'])
  assert.equal(def.code, 0)
  assert.deepEqual(trace, [`poll:${DEFAULT_POLL_WORKER_ID}:${DEFAULT_POLL_BATCH}`])
  assert.match(def.text, /poll pass/)

  trace.length = 0
  const explicit = await runWorkflowCliCore(makeDeps(trace), ['poll', 'worker-1', '3'])
  assert.equal(explicit.code, 0)
  assert.deepEqual(trace, ['poll:worker-1:3'])
})

test('reset:dev refuses without --yes and never touches the seam', async () => {
  const trace: string[] = []
  const refused = await runWorkflowCliCore(makeDeps(trace), ['reset:dev'])
  assert.equal(refused.code, 1)
  assert.deepEqual(trace, [], 'resetDev must not be invoked without confirmation')
  assert.match(refused.text, /--yes/)

  const confirmed = await runWorkflowCliCore(makeDeps(trace), ['reset:dev', '--yes'])
  assert.equal(confirmed.code, 0)
  assert.deepEqual(trace, ['resetDev'])
  assert.match(confirmed.text, /DEV workflow reset complete/)
})

test('test and test:persistence delegate to the pnpm scripts and propagate exit code', async () => {
  const trace: string[] = []
  const unit = await runWorkflowCliCore(makeDeps(trace), ['test'])
  assert.equal(unit.code, 0)
  assert.deepEqual(trace, ['pnpm:test'])

  const persistence = await runWorkflowCliCore(makeDeps(trace), ['test:persistence'])
  assert.equal(persistence.code, 2)
  assert.deepEqual(trace, ['pnpm:test', 'pnpm:test:persistence'])
})

test('unknown subcommand prints usage and exits 1', async () => {
  const trace: string[] = []
  const outcome = await runWorkflowCliCore(makeDeps(trace), ['start'])
  assert.equal(outcome.code, 1)
  assert.deepEqual(trace, [])
  assert.match(outcome.text, /Usage: pnpm workflow/)
})
