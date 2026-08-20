import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WorkflowEngine } from '../lib/workflow/engine';
import { FakeSql } from './fake-sql';
import { stubEvaluator } from './fixtures';

const LINEAR = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'to_a', to: 'a' }] },
    a: { id: 'a', type: 'state', transitions: [{ name: 'to_b', to: 'b' }] },
    b: { id: 'b', type: 'state', transitions: [{ name: 'to_end', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
};

const DECISION = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'd' }] },
    d: {
      id: 'd',
      type: 'decision',
      decisions: [
        { condition: 'amount > 100', transition: 'big' },
        { condition: 'amount <= 100', transition: 'small' },
      ],
      transitions: [
        { name: 'big', to: 'end_big' },
        { name: 'small', to: 'end_small' },
      ],
    },
    end_big: { id: 'end_big', type: 'end', name: 'Big' },
    end_small: { id: 'end_small', type: 'end', name: 'Small' },
  },
};

const TASK_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'review' }] },
    review: {
      id: 'review',
      type: 'task',
      name: 'Review',
      candidateGroups: ['bob'],
      transitions: [{ name: 'approve', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
};

const FORK_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
    f: {
      id: 'f',
      type: 'fork',
      transitions: [
        { name: 'a', to: 'taskA' },
        { name: 'b', to: 'taskB' },
      ],
    },
    taskA: { id: 'taskA', type: 'task', name: 'Task A', transitions: [{ name: 'done', to: 'endA' }] },
    taskB: { id: 'taskB', type: 'task', name: 'Task B', transitions: [{ name: 'done', to: 'endB' }] },
    endA: { id: 'endA', type: 'end' },
    endB: { id: 'endB', type: 'end' },
  },
};

const FORK_JOIN_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
    f: {
      id: 'f',
      type: 'fork',
      transitions: [
        { name: 'a', to: 'taskA' },
        { name: 'b', to: 'taskB' },
      ],
    },
    taskA: { id: 'taskA', type: 'task', name: 'Task A', transitions: [{ name: 'done', to: 'j' }] },
    taskB: { id: 'taskB', type: 'task', name: 'Task B', transitions: [{ name: 'done', to: 'j' }] },
    j: { id: 'j', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
};

function events(fake: FakeSql, type: string) {
  return fake.store.processEvents.filter((e) => e.event_type === type);
}

test('linear transition completes a process', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('linear', 1, LINEAR);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'linear',
    startedBy: 'alice',
  });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(pi!.outcome, 'completed');
  assert.equal(events(fake, 'process.completed').length, 1);
  assert.ok(events(fake, 'token.moved').length >= 3);
});

test('conditional transition routes on variables', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('dec', 1, DECISION);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const big = await engine.startProcess({
    definitionKey: 'dec',
    startedBy: 'alice',
    variables: { amount: 150 },
  });
  const small = await engine.startProcess({
    definitionKey: 'dec',
    startedBy: 'alice',
    variables: { amount: 50 },
  });

  const bigTokens = fake.store.tokens.filter((t) => t.process_instance_id === big.processInstanceId);
  const smallTokens = fake.store.tokens.filter((t) => t.process_instance_id === small.processInstanceId);
  assert.equal(bigTokens[bigTokens.length - 1].node_id, 'end_big');
  assert.equal(smallTokens[smallTokens.length - 1].node_id, 'end_small');
});

test('human task: created, claimed, completed', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TASK_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({
    definitionKey: 't',
    startedBy: 'alice',
  });

  let tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].status, 'ready');

  await engine.claimTask(tasks[0].id, 'bob');
  tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  assert.equal(tasks[0].status, 'reserved');
  assert.equal(tasks[0].assignee, 'bob');

  await engine.completeTask({
    taskId: tasks[0].id,
    userId: 'bob',
    transitionName: 'approve',
  });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(events(fake, 'task.claimed').length, 1);
  assert.equal(events(fake, 'task.completed').length, 1);
});

test('fork spawns one child token + task per branch', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('fork', 1, FORK_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'fork',
    startedBy: 'alice',
  });

  const tokens = fake.store.tokens.filter((t) => t.process_instance_id === processInstanceId);
  const tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  const childTokens = tokens.filter((t) => t.parent_token_id != null);
  assert.equal(childTokens.length, 2);
  assert.equal(tasks.length, 2);
  assert.equal(tokens.find((t) => t.parent_token_id == null)!.status, 'completed'); // fork token completed
});

test('join waits for all branches then releases', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('fj', 1, FORK_JOIN_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'fj',
    startedBy: 'alice',
  });

  const tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  assert.equal(tasks.length, 2);
  const taskA = tasks.find((t) => t.name === 'Task A')!;
  const taskB = tasks.find((t) => t.name === 'Task B')!;

  await engine.completeTask({ taskId: taskA.id, userId: 'w', transitionName: 'done' });
  let pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'active', 'join must wait for the other branch');
  assert.equal(events(fake, 'token.joined').length, 0);

  await engine.completeTask({ taskId: taskB.id, userId: 'w', transitionName: 'done' });
  pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(pi!.outcome, 'completed');
  assert.equal(events(fake, 'token.joined').length, 1);
});

test('job claim locks and increments attempts', async () => {
  const fake = new FakeSql();
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  await engine.createJob({
    type: 'async',
    dueAt: new Date(Date.now() - 1000),
    maxAttempts: 3,
  });

  const claimed = await engine.claimJobs('worker-1', 10);
  assert.equal(claimed.length, 1);
  assert.equal(claimed[0].status, 'locked');
  assert.equal(claimed[0].attempts, 1);
  assert.equal(claimed[0].lockedBy, 'worker-1');

  const none = await engine.claimJobs('worker-2', 10);
  assert.equal(none.length, 0, 'locked job must not be claimable by another worker');
});

test('job retry applies backoff and exhausts into failed', async () => {
  const fake = new FakeSql();
  let current = Date.now();
  const engine = new WorkflowEngine(fake.sql, {
    evaluate: stubEvaluator,
    now: () => new Date(current),
  });

  const jobId = await engine.createJob({
    type: 'async',
    dueAt: new Date(current - 1000),
    maxAttempts: 3,
  });

  await engine.claimJobs('w', 10);
  const beforeDue = fake.store.jobs.find((j) => j.id === jobId)!.due_at.getTime();
  await engine.failJob(jobId, 'w', 'boom');
  let job = fake.store.jobs.find((j) => j.id === jobId)!;
  assert.equal(job.status, 'pending');
  assert.equal(job.attempts, 1);
  assert.ok(job.due_at.getTime() > beforeDue, 'backoff must move due_at forward');

  current = job.due_at.getTime() + 1; // advance past backoff
  await engine.claimJobs('w', 10);
  await engine.failJob(jobId, 'w', 'boom');
  current = fake.store.jobs.find((j) => j.id === jobId)!.due_at.getTime() + 1;
  await engine.claimJobs('w', 10);
  await engine.failJob(jobId, 'w', 'boom');
  job = fake.store.jobs.find((j) => j.id === jobId)!;
  assert.equal(job.status, 'failed', 'attempts reached max_attempts');
  assert.equal(job.attempts, 3);
});
