import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  collectAnomalies,
  type AnomalyInstanceInput,
  type WorkflowAnomaly,
} from '../anomaly-core'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// TUNIT harvest mechanism #15 — terminal invariant sweep (GLOBAL INVARIANT).
//
// Every anomaly detector is exercised through the dependency-injected core
// with an in-memory fake executor; no DB/Neon required. The live sweep
// (workflow_app/diagnostics.ts) runs the same code against the real database.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeAnomalyDb {
  /** Rows returned per detector query, matched on normalized SQL text. */
  rowsByQuery: Array<{ match: string; rows: Row[] }> = []

  private norm(s: string): string {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    for (const entry of this.rowsByQuery) {
      if (t.includes(entry.match)) return Promise.resolve(entry.rows)
    }
    throw new Error(`FAKE_ANOMALY_DB_UNHANDLED: ${t}`)
  }
}

const instance = (over: Partial<AnomalyInstanceInput> = {}): AnomalyInstanceInput => ({
  instanceId: 'inst-1',
  definitionKey: 'RE_supermodel',
  definitionVersion: 1,
  subjectId: 'deal-1',
  status: 'active',
  outcome: null,
  ...over,
})

async function collect(
  f: FakeAnomalyDb,
  instances: AnomalyInstanceInput[],
): Promise<WorkflowAnomaly[]> {
  const anomalies: WorkflowAnomaly[] = []
  await collectAnomalies(anomalies, instances, f.tx)
  return anomalies
}

test('no violations → the sweep reports zero anomalies', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [instance()])
  assert.equal(anomalies.length, 0)
})

test('failed-process detector flags error status and failed outcome', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [
    instance({ instanceId: 'inst-error', status: 'error' }),
    instance({ instanceId: 'inst-failed', status: 'completed', outcome: 'failed' }),
    instance({ instanceId: 'inst-clean', status: 'completed', outcome: 'completed' }),
  ])
  assert.equal(anomalies.length, 2)
  assert.deepEqual(
    anomalies.map((a) => a.kind),
    ['failed-process', 'failed-process'],
  )
  assert.ok(
    anomalies.every((a) => a.severity === 'critical'),
  )
  assert.ok(anomalies.some((a) => a.message.includes('is in error state')))
  assert.ok(anomalies.some((a) => a.message.includes("terminated with outcome 'failed'")))
})

test('pending-receipt detector flags stuck pending receipts', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    {
      match: 'from workflow_command_receipt where outcome =',
      rows: [
        { command_id: 'cmd-1', aggregate_id: null, message: null },
        { command_id: 'cmd-2', aggregate_id: null, message: 'timeout' },
      ],
    },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 2)
  assert.ok(anomalies.every((a) => a.kind === 'pending-receipt' && a.severity === 'critical'))
  assert.ok(anomalies.some((a) => a.message.includes('cmd-1') && a.message.includes('stuck')))
  assert.ok(anomalies.some((a) => a.message.includes('cmd-2') && a.message.includes('— timeout')))
})

test('ready-task-uncorrelated detector flags engine tasks with no correlation', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    {
      match: 'from tasks t left join workflow_task_correlation',
      rows: [{ id: 'wt-1', pid: 'inst-1', name: 'Review docs', status: 'ready' }],
    },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 1)
  assert.equal(anomalies[0].kind, 'ready-task-uncorrelated')
  assert.equal(anomalies[0].severity, 'warning')
  assert.equal(anomalies[0].instanceId, 'inst-1')
  assert.ok(anomalies[0].message.includes('Review docs'))
})

test('correlation-dangling-app-task detector flags missing canonical task', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    {
      match: 'left join task t on t.id = c.application_task_id',
      rows: [{ workflow_task_id: 'wt-1', application_task_id: 'task-missing' }],
    },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 1)
  assert.equal(anomalies[0].kind, 'correlation-dangling-app-task')
  assert.equal(anomalies[0].subjectId, 'task-missing')
  assert.ok(anomalies[0].message.includes('wt-1'))
})

test('correlation-dangling-workflow-task detector flags missing engine task', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    {
      match: 'left join tasks t on t.id::text = c.workflow_task_id',
      rows: [{ workflow_task_id: 'wt-gone', application_task_id: 'task-1' }],
    },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 1)
  assert.equal(anomalies[0].kind, 'correlation-dangling-workflow-task')
  assert.ok(anomalies[0].message.includes('wt-gone'))
})

test('open-job-on-closed-token detector flags stale jobs', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    {
      match: 'from jobs j join tokens t on t.id = j.token_id',
      rows: [
        { job_id: 'job-1', type: 'timer', status: 'pending', pid: 'inst-1', token_id: 'tok-1', token_outcome: 'completed' },
      ],
    },
    { match: 'from process_instances where status =', rows: [] },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 1)
  assert.equal(anomalies[0].kind, 'open-job-on-closed-token')
  assert.equal(anomalies[0].severity, 'warning')
  assert.equal(anomalies[0].instanceId, 'inst-1')
  assert.ok(anomalies[0].message.includes('job-1'))
})

test('multiple-active-instances detector flags duplicate active instances', async () => {
  const f = new FakeAnomalyDb()
  f.rowsByQuery = [
    { match: 'from workflow_command_receipt where outcome =', rows: [] },
    { match: 'from tasks t left join workflow_task_correlation', rows: [] },
    { match: 'left join task t on t.id = c.application_task_id', rows: [] },
    { match: 'left join tasks t on t.id::text = c.workflow_task_id', rows: [] },
    { match: 'from jobs j join tokens t on t.id = j.token_id', rows: [] },
    {
      match: 'from process_instances where status =',
      rows: [{ subject_type: 'deal', subject_id: 'deal-9', c: 2 }],
    },
  ]
  const anomalies = await collect(f, [])
  assert.equal(anomalies.length, 1)
  assert.equal(anomalies[0].kind, 'multiple-active-instances')
  assert.equal(anomalies[0].subjectId, 'deal-9')
  assert.ok(anomalies[0].message.includes('deal:deal-9'))
})
