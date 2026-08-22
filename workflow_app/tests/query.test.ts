import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  queryWorkflowInstance,
  milestoneState,
  optionalNodeIds,
} from '../query'
import { classifyInstanceHealth } from '../anomaly-core'
import type { QueryExecutor } from '../../db/query-executor'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-15 — Workflow Instance Query / History / Health.
//
// The query contract is dependency-injected (same pattern as anomaly-core):
// every read goes through the injected `execute` handle, so the full contract
// is exercised against an in-memory fake with no DB/Neon required. The live
// path (workflow_app/query.ts default handle) runs the same code against the
// existing canonical engine tables — no projection is written anywhere.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeQueryDb {
  /** Rows returned per query, matched on normalized SQL text (ordered). */
  entries: Array<{ match: string; rows: Row[] }> = []

  add(match: string, rows: Row[]): this {
    this.entries.push({ match, rows })
    return this
  }

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
    for (const entry of this.entries) {
      if (t.includes(entry.match)) return Promise.resolve(entry.rows)
    }
    throw new Error(`FAKE_QUERY_DB_UNHANDLED: ${t}`)
  }
}

const NOW = () => new Date('2026-08-01T00:00:00Z')

const GRAPH: ProcessGraph = {
  startNodeId: 'tA',
  nodes: {
    tA: {
      id: 'tA',
      type: 'task',
      name: 'Appraisal',
      description: 'Order the appraisal',
      responsibility: 'appraisal',
    },
    tB: { id: 'tB', type: 'task', name: 'Review docs' },
    tB_blocker: { id: 'tB_blocker', type: 'task', name: 'Blocker: docs' },
    tDecision: {
      id: 'tDecision',
      type: 'decision',
      transitions: [
        { name: 'default', to: 'tA' },
        { name: 'escalate', to: 'tB' },
      ],
      decisions: [{ condition: 'x', transition: 'escalate' }],
    },
    tEnd: { id: 'tEnd', type: 'end' },
  },
  displayOrder: ['tA', 'tB', 'tEnd'],
}

const INSTANCE_ROW: Row = {
  instance_id: 'inst-1',
  subject_type: 'deal',
  subject_id: 'deal-9',
  status: 'active',
  outcome: null,
  variables: { dealId: 'deal-9' },
  started_at: '2026-08-01T00:00:00Z',
  ended_at: null,
  key: 'RE_supermodel',
  version: 1,
  name: 'RE Supermodel',
  definition: GRAPH,
}

/** A fake pre-loaded for a healthy active instance. */
function healthyDb(): FakeQueryDb {
  const f = new FakeQueryDb()
  f.add('from process_instances pi', [INSTANCE_ROW])
    .add('from workflow_task_correlation c', [])
    .add('from tokens where process_instance_id', [
      { id: 'tok-1', node_id: 'tA', status: 'active', outcome: null },
    ])
    .add('as due_date from tasks where process_instance_id', [
      {
        id: 'task-1',
        token_id: 'tok-1',
        name: 'Appraisal',
        status: 'ready',
        candidates: ['ops'],
        assignee: null,
        due_date: null,
      },
    ])
    .add('from jobs where process_instance_id', [
      {
        id: 'job-1',
        token_id: null,
        type: 'timer',
        status: 'pending',
        due_at: '2026-09-01T00:00:00Z',
        attempts: 0,
        max_attempts: 5,
        last_error: null,
        locked_by: null,
        locked_until: null,
      },
    ])
    .add('from process_commands', [])
    .add('from workflow_command_receipt', [])
    .add('from process_events', [
      {
        id: '2',
        event_type: 'task.completed',
        node_id: 'tA',
        actor: 'alice',
        data: {},
        created_at: '2026-08-02T00:00:00Z',
      },
      {
        id: '1',
        event_type: 'process.started',
        node_id: null,
        actor: 'seed',
        data: { key: 'RE_supermodel' },
        created_at: '2026-08-01T00:00:00Z',
      },
    ])
  return f
}

test('query returns the coherent contract for a healthy active instance', async () => {
  const q = await queryWorkflowInstance('inst-1', healthyDb().tx, NOW)
  assert.ok(q)

  // Where it is.
  assert.equal(q.instance.definitionKey, 'RE_supermodel')
  assert.equal(q.instance.definitionVersion, 1)
  assert.equal(q.instance.subjectType, 'deal')
  assert.equal(q.instance.subjectId, 'deal-9')
  assert.deepEqual(q.instance.currentNodeIds, ['tA'])
  assert.deepEqual(q.instance.activeMilestones, [
    { nodeId: 'tA', label: 'Appraisal', responsibility: 'appraisal' },
  ])
  assert.equal(q.instance.nodeLabels['tB'], 'Review docs')
  assert.equal(q.instance.nodeDescriptions['tA'], 'Order the appraisal')
  assert.equal(q.instance.nodeResponsibility['tA'], 'appraisal')
  assert.deepEqual(q.instance.displayOrder, ['tA', 'tB', 'tEnd'])
  assert.deepEqual(q.instance.optionalNodes, ['tB'])
  assert.deepEqual(q.instance.variables, { dealId: 'deal-9' })

  // What is active / waiting / retrying / failed.
  assert.deepEqual(q.state, {
    active: 1,
    waiting: 2,
    retrying: 0,
    failed: 0,
    activeTokens: 1,
    readyTasks: 1,
    inProgressTasks: 0,
    lockedJobs: 0,
    pendingJobs: 1,
    retryingJobs: 0,
    failedTasks: 0,
    failedJobs: 0,
    failedCommands: 0,
  })

  // What tasks / jobs / timers exist.
  assert.equal(q.work.tasks.length, 1)
  assert.equal(q.work.tasks[0].nodeId, 'tA')
  assert.equal(q.work.tasks[0].tokenId, 'tok-1')
  assert.equal(q.work.jobs.length, 1)
  assert.equal(q.work.timers.length, 1)
  assert.equal(q.work.timers[0].type, 'timer')

  // What happened historically (newest first).
  assert.deepEqual(
    q.history.map((e) => e.eventType),
    ['task.completed', 'process.started'],
  )
  assert.equal(q.history[1].data['key'], 'RE_supermodel')

  // Health.
  assert.equal(q.health.classification, 'healthy')
  assert.deepEqual(q.health.reasons, [])
})

test('query maps retrying and failed work into the state rollup', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from jobs where process_instance_id')
  f.add('from jobs where process_instance_id', [
    {
      id: 'job-r',
      token_id: null,
      type: 'async',
      status: 'pending',
      due_at: null,
      attempts: 2,
      max_attempts: 5,
      last_error: 'boom',
      locked_by: null,
      locked_until: null,
    },
    {
      id: 'job-f',
      token_id: null,
      type: 'message',
      status: 'failed',
      due_at: null,
      attempts: 5,
      max_attempts: 5,
      last_error: 'exhausted',
      locked_by: null,
      locked_until: null,
    },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'as due_date from tasks where process_instance_id')
  f.add('as due_date from tasks where process_instance_id', [
    { id: 'task-f', token_id: null, name: 'Sign', status: 'failed', candidates: [], assignee: null, due_date: null },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'from process_commands')
  f.add('from process_commands', [
    { command_id: 'cmd-1', command_type: 'setStage', node_id: 'tC', outcome: 'conflict', message: 'nope' },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'from workflow_command_receipt')
  f.add('from workflow_command_receipt', [{ command_id: 'cmd-1', outcome: 'success', message: null }])

  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.state.retrying, 1)
  assert.equal(q.state.failed, 3) // failed task + failed job + failed command
  assert.equal(q.state.retryingJobs, 1)
  assert.equal(q.state.failedJobs, 1)
  assert.equal(q.state.failedTasks, 1)
  assert.equal(q.state.failedCommands, 1)
  // Failed work that is not an operational anomaly keeps the instance healthy.
  assert.equal(q.health.classification, 'healthy')
  // Receipt outcome surfaces on the command view.
  assert.equal(q.work.commands[0].receiptOutcome, 'success')
})

test('query classifies a wedged instance as stuck', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from tokens where process_instance_id')
  f.add('from tokens where process_instance_id', [])
  f.entries = f.entries.filter((e) => e.match !== 'from jobs where process_instance_id')
  f.add('from jobs where process_instance_id', [])

  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'stuck')
  assert.equal(q.health.anomalies[0].kind, 'wedged-instance')
  assert.ok(q.health.reasons.some((r) => r.includes('no active tokens or pending work')))
})

test('query classifies an instance with a stale locked job as stuck', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from jobs where process_instance_id')
  f.add('from jobs where process_instance_id', [
    {
      id: 'job-9',
      token_id: null,
      type: 'timer',
      status: 'locked',
      due_at: null,
      attempts: 1,
      max_attempts: 5,
      last_error: null,
      locked_by: 'dead-worker',
      locked_until: '2026-07-01T00:00:00Z',
    },
  ])

  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'stuck')
  assert.ok(q.health.reasons.some((r) => r.includes('past its lease')))
})

test('query classifies an open job on a closed token as stuck', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from tokens where process_instance_id')
  f.add('from tokens where process_instance_id', [
    { id: 'tok-c', node_id: 'tB', status: 'completed', outcome: 'completed' },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'from jobs where process_instance_id')
  f.add('from jobs where process_instance_id', [
    {
      id: 'job-c',
      token_id: 'tok-c',
      type: 'async',
      status: 'pending',
      due_at: null,
      attempts: 0,
      max_attempts: 5,
      last_error: null,
      locked_by: null,
      locked_until: null,
    },
  ])

  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'stuck')
  assert.equal(q.health.anomalies[0].kind, 'open-job-on-closed-token')
})

test('query classifies a poisoned command receipt as stuck', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from process_commands')
  f.add('from process_commands', [
    { command_id: 'cmd-9', command_type: 'setStage', node_id: 'tC', outcome: 'success', message: null },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'from workflow_command_receipt')
  f.add('from workflow_command_receipt', [
    { command_id: 'cmd-9', outcome: 'pending', message: 'timeout' },
  ])

  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'stuck')
  assert.equal(q.health.anomalies[0].kind, 'pending-receipt')
  assert.equal(q.work.commands[0].receiptOutcome, 'pending')
})

test('query classifies error and failed-outcome instances as failed', async () => {
  for (const over of [
    { status: 'error', outcome: null },
    { status: 'completed', outcome: 'failed' },
  ]) {
    const f = healthyDb()
    f.entries = f.entries.filter((e) => e.match !== 'from process_instances pi')
    f.add('from process_instances pi', [{ ...INSTANCE_ROW, ...over }])
    const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
    assert.ok(q)
    assert.equal(q.health.classification, 'failed')
    assert.ok(q.health.anomalies.some((a) => a.kind === 'failed-process'))
  }
})

test('query classifies an error instance with no outcome as failed', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from process_instances pi')
  f.add('from process_instances pi', [{ ...INSTANCE_ROW, status: 'error', outcome: null }])
  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'failed')
  assert.ok(q.health.anomalies.some((a) => a.kind === 'error-instance-missing-outcome'))
})

test('query classifies a normally terminal instance as terminal', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from process_instances pi')
  f.add('from process_instances pi', [
    { ...INSTANCE_ROW, status: 'completed', outcome: 'completed', ended_at: '2026-08-03T00:00:00Z' },
  ])
  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.equal(q.health.classification, 'terminal')
  assert.equal(q.instance.endedAt, '2026-08-03T00:00:00Z')
})

test('query reports orphan tokens on a terminal instance', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from process_instances pi')
  f.add('from process_instances pi', [
    { ...INSTANCE_ROW, status: 'completed', outcome: 'completed' },
  ])
  f.entries = f.entries.filter((e) => e.match !== 'from tokens where process_instance_id')
  f.add('from tokens where process_instance_id', [
    { id: 'tok-x', node_id: 'tA', status: 'active', outcome: null },
  ])
  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  // A finished instance is never "stuck", but the orphan token is surfaced.
  assert.equal(q.health.classification, 'terminal')
  assert.ok(q.health.anomalies.some((a) => a.kind === 'orphan-token'))
})

test('query returns null for an unknown instance', async () => {
  const f = new FakeQueryDb().add('from process_instances pi', [])
  const q = await queryWorkflowInstance('inst-missing', f.tx, NOW)
  assert.equal(q, null)
})

test('query maps correlations to the canonical task', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from workflow_task_correlation c')
  f.add('from workflow_task_correlation c', [
    {
      workflow_task_id: 'task-1',
      application_task_id: 'canonical-1',
      application_task_status: 'open',
      application_task_title: 'Order appraisal',
    },
  ])
  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.deepEqual(q.work.correlations, [
    {
      workflowTaskId: 'task-1',
      applicationTaskId: 'canonical-1',
      applicationTaskStatus: 'open',
      applicationTaskTitle: 'Order appraisal',
    },
  ])
})

test('query distinguishes blockers and optional nodes via shared helpers', async () => {
  const f = healthyDb()
  f.entries = f.entries.filter((e) => e.match !== 'from tokens where process_instance_id')
  f.add('from tokens where process_instance_id', [
    { id: 'tok-b', node_id: 'tB_blocker', status: 'active', outcome: null },
  ])
  const q = await queryWorkflowInstance('inst-1', f.tx, NOW)
  assert.ok(q)
  assert.deepEqual(q.instance.blockerLabels, ['Blocker: docs'])
  assert.deepEqual(q.instance.currentNodeIds, ['tB_blocker'])
  // Optional node analysis: non-default decision transition target.
  assert.deepEqual(optionalNodeIds(GRAPH), ['tB'])
  // Milestone classifier excludes blockers from active milestones.
  const ms = milestoneState(
    [{ id: 'tok-b', node_id: 'tB_blocker', status: 'active', outcome: null }],
    GRAPH,
  )
  assert.deepEqual([...ms.blockerNodeIds], ['tB_blocker'])
  assert.equal(ms.activeNodeIds.size, 0)
})

test('classifyInstanceHealth ordering: failed dominates, terminal is never stuck', () => {
  const anomaly = (kind: string) => ({
    kind,
    severity: 'warning' as const,
    instanceId: 'inst-1',
    subjectId: null,
    message: `anomaly: ${kind}`,
  })

  assert.equal(
    classifyInstanceHealth({
      status: 'active',
      outcome: null,
      anomalies: [anomaly('wedged-instance'), anomaly('failed-process')],
    }).classification,
    'failed',
  )
  assert.equal(
    classifyInstanceHealth({
      status: 'completed',
      outcome: 'completed',
      anomalies: [anomaly('orphan-token')],
    }).classification,
    'terminal',
  )
  assert.equal(
    classifyInstanceHealth({
      status: 'active',
      outcome: null,
      anomalies: [anomaly('stale-locked-job')],
    }).classification,
    'stuck',
  )
  assert.equal(
    classifyInstanceHealth({
      status: 'active',
      outcome: null,
      anomalies: [],
    }).classification,
    'healthy',
  )
})
