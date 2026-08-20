import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WorkflowEngine } from '../lib/workflow/engine';
import { StaleTokenError } from '../lib/workflow/errors';
import { FakeSql } from './fake-sql';
import { stubEvaluator, makeApp } from './fixtures';

// Generic fixture (no real-estate terminology): human gate → 4-branch fork
// (one optional) → join → timer → decision(fact refresh) → command → success.
const TORTURE = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'approve' }] },
    approve: { id: 'approve', type: 'task', name: 'Approve', transitions: [{ name: 'ok', to: 'fork' }] },
    fork: {
      id: 'fork',
      type: 'fork',
      transitions: [
        { name: 'b1', to: 'work1' },
        { name: 'b2', to: 'work2' },
        { name: 'b3', to: 'work3' },
        { name: 'b4', to: 'work4', required: false },
      ],
    },
    work1: { id: 'work1', type: 'task', name: 'Work1', transitions: [{ name: 'done', to: 'join' }] },
    work2: { id: 'work2', type: 'task', name: 'Work2', transitions: [{ name: 'done', to: 'join' }] },
    work3: { id: 'work3', type: 'task', name: 'Work3', transitions: [{ name: 'done', to: 'join' }] },
    work4: { id: 'work4', type: 'task', name: 'Work4', transitions: [{ name: 'done', to: 'join' }] },
    join: { id: 'join', type: 'join', transitions: [{ name: 'go', to: 'wait' }] },
    wait: {
      id: 'wait',
      type: 'timer',
      timer: { dueAtVariable: 'deadline', transition: 'fire' },
      transitions: [{ name: 'fire', to: 'decide' }],
    },
    decide: {
      id: 'decide',
      type: 'decision',
      decisions: [{ condition: "stage == 'ready'", transition: 'toCmd' }],
      transitions: [
        { name: 'toCmd', to: 'cmd' },
        { name: 'other', to: 'done' },
      ],
    },
    cmd: { id: 'cmd', type: 'command', commandType: 'c.do', transition: 'go', transitions: [{ name: 'go', to: 'done' }] },
    done: { id: 'done', type: 'end' },
  },
};

function events(fake: FakeSql, type: string) {
  return fake.store.processEvents.filter((e) => e.event_type === type);
}

async function runFullPath() {
  const fake = new FakeSql();
  fake.seedDefinition('torture', 1, TORTURE);
  const app = makeApp({ readFacts: async () => ({ stage: 'ready' }) });
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });

  const past = new Date(Date.now() - 1000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'torture',
    startedBy: 'x',
    variables: { deadline: past },
    subject: { subjectType: 'deal', subjectId: 'd1' },
  });

  const approve = fake.store.tasks.find((t) => t.name === 'Approve')!;
  await engine.completeTask({ taskId: approve.id, userId: 'w', transitionName: 'ok' });

  for (const name of ['Work1', 'Work2', 'Work3']) {
    const t = fake.store.tasks.find((x) => x.name === name)!;
    await engine.completeTask({ taskId: t.id, userId: 'w', transitionName: 'done' });
  }

  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;
  await engine.claimJobs('worker', 10);
  await engine.fireTimerJob({ jobId: job.id, workerId: 'worker' });

  return { fake, engine, app, processInstanceId };
}

test('full path: gate → parallel fork → optional skip → join → timer → decision → command → success', async () => {
  const { fake, engine, app, processInstanceId } = await runFullPath();

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(pi!.outcome, 'completed');

  const work4 = fake.store.tasks.find((t) => t.name === 'Work4')!;
  assert.equal(work4.status, 'obsolete', 'optional branch task must be obsoleted');

  assert.equal(app.calls.length, 1, 'command executed exactly once');
  assert.ok(app.factCalls.length >= 1, 'facts were refreshed before the decision');

  for (const type of [
    'process.started',
    'task.created',
    'token.forked',
    'token.skipped',
    'token.joined',
    'timer.scheduled',
    'timer.fired',
    'command.requested',
    'command.completed',
    'process.completed',
  ]) {
    assert.ok(events(fake, type).length >= 1, `expected at least one ${type} event`);
  }

  const commandRecord = fake.store.processCommands.find((r) => r.process_instance_id === processInstanceId)!;
  assert.equal(commandRecord.outcome, 'success');
  assert.equal(commandRecord.command_type, 'c.do');
});

test('required branch cancelled terminates the process as cancelled', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
      f: {
        id: 'f',
        type: 'fork',
        transitions: [
          { name: 'a', to: 'taskA' },
          { name: 'b', to: 'end_cancelled' },
        ],
      },
      taskA: { id: 'taskA', type: 'task', name: 'A', transitions: [{ name: 'done', to: 'end' }] },
      end_cancelled: { id: 'end_cancelled', type: 'end', outcome: 'cancelled' },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'cancelled');
  assert.equal(pi!.status, 'aborted');
});

test('timer retry: fail the firing once, then succeed', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('torture', 1, TORTURE);
  const app = makeApp({ readFacts: async () => ({ stage: 'ready' }) });
  let current = Date.now();
  const engine = new WorkflowEngine(fake.sql, {
    evaluate: stubEvaluator,
    app,
    now: () => new Date(current),
  });

  const past = new Date(current - 1000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'torture',
    startedBy: 'x',
    variables: { deadline: past },
    subject: { subjectType: 'deal', subjectId: 'd1' },
  });

  const approve = fake.store.tasks.find((t) => t.name === 'Approve')!;
  await engine.completeTask({ taskId: approve.id, userId: 'w', transitionName: 'ok' });
  for (const name of ['Work1', 'Work2', 'Work3']) {
    const t = fake.store.tasks.find((x) => x.name === name)!;
    await engine.completeTask({ taskId: t.id, userId: 'w', transitionName: 'done' });
  }

  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;
  await engine.claimJobs('worker', 10);
  await engine.failJob(job.id, 'worker', 'transient'); // retry scheduled
  assert.equal(events(fake, 'job.retry_scheduled').length, 1);

  current = fake.store.jobs.find((j) => j.id === job.id)!.due_at.getTime() + 1; // advance past backoff
  await engine.claimJobs('worker', 10);
  await engine.fireTimerJob({ jobId: job.id, workerId: 'worker' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'completed');
});

test('duplicate timer fire is rejected (retry safety)', async () => {
  const { fake, engine, processInstanceId } = await runFullPath();
  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;
  await assert.rejects(
    engine.fireTimerJob({ jobId: job.id, workerId: 'worker' }),
    (err: any) => err.code === 'TIMER_ALREADY_FIRED',
  );
});

test('duplicate task completion is rejected (retry safety)', async () => {
  const { fake, engine } = await runFullPath();
  const done = fake.store.tasks.find((t) => t.status === 'completed')!;
  await assert.rejects(
    engine.completeTask({ taskId: done.id, userId: 'w' }),
    /cannot be completed/,
  );
});

test('stale token conflict raises deterministic error', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('torture', 1, TORTURE);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const past = new Date(Date.now() - 1000).toISOString();
  const { rootTokenId } = await engine.startProcess({
    definitionKey: 'torture',
    startedBy: 'x',
    variables: { deadline: past },
  });
  const token = await engine.getToken(rootTokenId);

  await assert.rejects(
    (engine as any)._moveToken(fake.sql, { ...token, version: 999 }, 'x', 't', 'x'),
    (err: any) => err instanceof StaleTokenError,
  );
});

test('process cancellation mid-flight closes everything', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('torture', 1, TORTURE);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const past = new Date(Date.now() - 1000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'torture',
    startedBy: 'x',
    variables: { deadline: past },
  });
  const approve = fake.store.tasks.find((t) => t.name === 'Approve')!;
  await engine.completeTask({ taskId: approve.id, userId: 'w', transitionName: 'ok' });

  await engine.cancelProcess({ processInstanceId, actor: 'boss' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'cancelled');
  const workTasks = fake.store.tasks.filter(
    (t) => t.process_instance_id === processInstanceId && t.name.startsWith('Work'),
  );
  assert.ok(workTasks.length > 0);
  assert.ok(workTasks.every((t) => t.status === 'obsolete'));
});

test('explicit failure terminal', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end', outcome: 'failed' },
    },
  });
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });
  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'failed');
});

test('explicit conflict terminal', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end', outcome: 'conflict' },
    },
  });
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });
  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'conflict');
});
