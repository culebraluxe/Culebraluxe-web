import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine';
import { StaleTokenError, WorkflowConflictError } from '../../workflow_engine/lib/workflow/errors';
import { FakeSql } from './fake-sql';
import { stubEvaluator, makeApp } from './fixtures';

function endGraph(outcome?: string) {
  return {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end', outcome },
    },
  };
}

const TASK_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'review' }] },
    review: { id: 'review', type: 'task', name: 'Review', transitions: [{ name: 'done', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
};

const TIMER_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'wait' }] },
    wait: {
      id: 'wait',
      type: 'timer',
      timer: { dueAtVariable: 'deadline', transition: 'fire' },
      transitions: [{ name: 'fire', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
};

const COMMAND_GRAPH = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
    cmd: {
      id: 'cmd',
      type: 'command',
      commandType: 'c.do',
      transition: 'go',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
};

function events(fake: FakeSql, type: string) {
  return fake.store.processEvents.filter((e) => e.event_type === type);
}

// ---------------------------------------------------------------------------
// Story 2/3 — terminal outcome model + terminal node semantics
// ---------------------------------------------------------------------------

test('end node with outcome "failed" terminates with failed', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, endGraph('failed'));
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'error');
  assert.equal(pi!.outcome, 'failed');
  assert.equal(events(fake, 'process.failed').length, 1);
});

test('end node with outcome "conflict" terminates with conflict', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, endGraph('conflict'));
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'error');
  assert.equal(pi!.outcome, 'conflict');
  assert.equal(events(fake, 'process.conflict').length, 1);
});

test('end node with outcome "cancelled" terminates with cancelled', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, endGraph('cancelled'));
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'aborted');
  assert.equal(pi!.outcome, 'cancelled');
  assert.equal(events(fake, 'process.cancelled').length, 1);
});

test('default end node outcome is completed', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, endGraph());
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(pi!.outcome, 'completed');
});

// ---------------------------------------------------------------------------
// Story 4 — cancel / abort
// ---------------------------------------------------------------------------

test('cancelProcess closes tokens/tasks/jobs and is idempotent', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TASK_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 't', startedBy: 'x' });

  await engine.cancelProcess({ processInstanceId, actor: 'boss', reason: 'client withdrew' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'aborted');
  assert.equal(pi!.outcome, 'cancelled');

  const tokens = fake.store.tokens.filter((t) => t.process_instance_id === processInstanceId);
  assert.ok(tokens.every((t) => t.status === 'completed'));
  assert.ok(tokens.some((t) => t.outcome === 'cancelled'));

  const tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  assert.ok(tasks.every((t) => t.status === 'obsolete'));
  assert.equal(events(fake, 'process.cancelled').length, 1);

  await engine.cancelProcess({ processInstanceId, actor: 'boss' }); // idempotent, no throw
});

test('cancelProcess cannot resurrect a completed process', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('g', 1, endGraph());
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  await assert.rejects(
    engine.cancelProcess({ processInstanceId, actor: 'boss' }),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'PROCESS_NOT_ACTIVE',
  );
});

// ---------------------------------------------------------------------------
// Story ENG-13 — generic human-task lifecycle
// (claim / release / reassign / complete mechanics + deterministic conflicts)
// ---------------------------------------------------------------------------

const TASK_GRAPH_CANDIDATES = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'review' }] },
    review: {
      id: 'review',
      type: 'task',
      name: 'Review',
      candidateGroups: ['u1', 'u2'],
      transitions: [{ name: 'done', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
};

function taskRow(fake: FakeSql, processInstanceId: string) {
  return fake.store.tasks.find((t) => t.process_instance_id === processInstanceId)!;
}

test('ENG-13: claim -> release -> reassign -> complete lifecycle (deterministic unit)', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TASK_GRAPH_CANDIDATES);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 't', startedBy: 'x' });
  const taskId = taskRow(fake, processInstanceId).id;

  // Claim by u1.
  await engine.claimTask(taskId, 'u1');
  let task = taskRow(fake, processInstanceId);
  assert.equal(task.status, 'reserved');
  assert.equal(task.assignee, 'u1');
  assert.equal(events(fake, 'task.claimed').length, 1);

  // Only the assignee may release; release returns the task to ready.
  await assert.rejects(
    engine.releaseTask(taskId, 'u2'),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_ASSIGNEE_ONLY',
  );
  await engine.releaseTask(taskId, 'u1');
  task = taskRow(fake, processInstanceId);
  assert.equal(task.status, 'ready');
  assert.equal(task.assignee, null);
  assert.equal(events(fake, 'task.released').length, 1);

  // Reassign pre-assigns the ready task to u2 (recorded from null).
  await engine.reassignTask(taskId, 'u2', 'manager');
  task = taskRow(fake, processInstanceId);
  assert.equal(task.status, 'reserved');
  assert.equal(task.assignee, 'u2');
  const reassigned = events(fake, 'task.reassigned');
  assert.equal(reassigned.length, 1);
  assert.equal(reassigned[0].data.from, null);
  assert.equal(reassigned[0].data.to, 'u2');
  assert.equal(reassigned[0].actor, 'manager');

  // A non-assignee cannot complete; the assignee completes exactly once.
  await assert.rejects(
    engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_ASSIGNEE_ONLY',
  );
  await engine.completeTask({ taskId, userId: 'u2', transitionName: 'done' });
  task = taskRow(fake, processInstanceId);
  assert.equal(task.status, 'completed');
  assert.equal(task.completed_by, 'u2');
  assert.equal(events(fake, 'task.completed').length, 1);

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
});

test('ENG-13: reassign validates eligibility and refuses terminal tasks', async () => {
  // A fork keeps the process active after one branch completes, so a
  // completed task can be observed on an ACTIVE instance — reassign must
  // report TASK_ALREADY_COMPLETED there (not a process-level conflict).
  const forkGraph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'fork' }] },
      fork: {
        id: 'fork',
        type: 'fork',
        transitions: [
          { name: 'a', to: 'tA', required: true },
          { name: 'b', to: 'tB', required: true },
        ],
      },
      tA: {
        id: 'tA',
        type: 'task',
        name: 'A',
        candidateGroups: ['u1', 'u2'],
        transitions: [{ name: 'done', to: 'endA' }],
      },
      tB: {
        id: 'tB',
        type: 'task',
        name: 'B',
        candidateGroups: ['u1', 'u2'],
        transitions: [{ name: 'done', to: 'endB' }],
      },
      endA: { id: 'endA', type: 'end' },
      endB: { id: 'endB', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('t', 1, forkGraph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 't', startedBy: 'x' });
  const tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  const taskA = tasks.find((t) => t.name === 'A')!;

  // Outsider is not a candidate.
  await assert.rejects(
    engine.reassignTask(taskA.id, 'outsider', 'manager'),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_CANDIDATE_ONLY',
  );

  // Complete branch A: process stays active (fork waits for B).
  await engine.completeTask({ taskId: taskA.id, userId: 'u1', transitionName: 'done' });
  assert.equal(
    (await engine.getProcessInstance(processInstanceId))!.status,
    'active',
    'fork keeps the process active after one branch completes',
  );

  // Reassigning a completed task is TASK_ALREADY_COMPLETED; a duplicate
  // completion of the same task is TASK_ALREADY_COMPLETED too.
  await assert.rejects(
    engine.reassignTask(taskA.id, 'u2', 'manager'),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_ALREADY_COMPLETED',
  );
  await assert.rejects(
    engine.completeTask({ taskId: taskA.id, userId: 'u1', transitionName: 'done' }),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_ALREADY_COMPLETED',
  );
  assert.equal(events(fake, 'task.completed').length, 1, 'completion recorded exactly once');
});

test('ENG-13: claim/release/complete/reassign on an obsoleted (terminal) task are conflicts', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TASK_GRAPH_CANDIDATES);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 't', startedBy: 'x' });
  const taskId = taskRow(fake, processInstanceId).id;

  await engine.cancelProcess({ processInstanceId, actor: 'boss' });
  assert.equal(taskRow(fake, processInstanceId).status, 'obsolete');

  // Pre-assignment ops guard the instance first: a dead process is a
  // PROCESS_NOT_ACTIVE conflict. Completion reports the task-level status
  // first: an obsolete task is TASK_NOT_ACTIONABLE. Both are deterministic.
  await assert.rejects(
    engine.claimTask(taskId, 'u1'),
    (err: any) => err.code === 'PROCESS_NOT_ACTIVE',
  );
  await assert.rejects(
    engine.releaseTask(taskId, 'u1'),
    (err: any) => err.code === 'PROCESS_NOT_ACTIVE',
  );
  await assert.rejects(
    engine.reassignTask(taskId, 'u2', 'manager'),
    (err: any) => err.code === 'PROCESS_NOT_ACTIVE',
  );
  await assert.rejects(
    engine.completeTask({ taskId, userId: 'u1', transitionName: 'done' }),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TASK_NOT_ACTIONABLE',
  );
});

// ---------------------------------------------------------------------------
// Story 5 — optimistic token guard
// ---------------------------------------------------------------------------

test('stale token move raises deterministic conflict', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TASK_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { rootTokenId } = await engine.startProcess({ definitionKey: 't', startedBy: 'x' });
  const token = await engine.getToken(rootTokenId);

  await assert.rejects(
    (engine as any)._moveToken(fake.sql, { ...token, version: 999 }, 'somewhere', 't', 'x'),
    (err: any) => err instanceof StaleTokenError && err.code === 'STALE_TOKEN',
  );
});

// ---------------------------------------------------------------------------
// Story 6/7/8 — branch outcomes and join policy
// ---------------------------------------------------------------------------

test('required branch failure terminates the process and cancels siblings', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
      f: {
        id: 'f',
        type: 'fork',
        transitions: [
          { name: 'safe', to: 'taskSafe' },
          { name: 'fail', to: 'end_failed' },
        ],
      },
      taskSafe: { id: 'taskSafe', type: 'task', name: 'Safe', transitions: [{ name: 'done', to: 'end' }] },
      end_failed: { id: 'end_failed', type: 'end', outcome: 'failed' },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'failed');
  assert.equal(pi!.status, 'error');

  const safeTask = fake.store.tasks.find((t) => t.name === 'Safe' && t.process_instance_id === processInstanceId);
  assert.equal(safeTask!.status, 'obsolete');
  assert.equal(events(fake, 'process.failed').length, 1);
});

test('optional branch reaching a cancelled end node does not terminate the process', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
      f: {
        id: 'f',
        type: 'fork',
        transitions: [
          { name: 'req', to: 'taskReq' },
          { name: 'opt', to: 'end_cancelled', required: false },
        ],
      },
      taskReq: { id: 'taskReq', type: 'task', name: 'Req', transitions: [{ name: 'done', to: 'j' }] },
      end_cancelled: { id: 'end_cancelled', type: 'end', outcome: 'cancelled' },
      j: { id: 'j', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });

  const reqTask = fake.store.tasks.find((t) => t.name === 'Req' && t.process_instance_id === processInstanceId)!;
  await engine.completeTask({ taskId: reqTask.id, userId: 'w', transitionName: 'done' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'completed');

  const optToken = fake.store.tokens.find((t) => t.process_instance_id === processInstanceId && t.node_id === 'end_cancelled')!;
  assert.equal(optToken.outcome, 'cancelled');
});

test('optional branch still active at join release is skipped (and its task obsoleted)', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
      f: {
        id: 'f',
        type: 'fork',
        transitions: [
          { name: 'req', to: 'taskReq' },
          { name: 'opt', to: 'taskOpt', required: false },
        ],
      },
      taskReq: { id: 'taskReq', type: 'task', name: 'Req', transitions: [{ name: 'done', to: 'j' }] },
      taskOpt: { id: 'taskOpt', type: 'task', name: 'Opt', transitions: [{ name: 'done', to: 'j' }] },
      j: { id: 'j', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });

  const reqTask = fake.store.tasks.find((t) => t.name === 'Req' && t.process_instance_id === processInstanceId)!;
  await engine.completeTask({ taskId: reqTask.id, userId: 'w', transitionName: 'done' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'completed');

  const optTask = fake.store.tasks.find((t) => t.name === 'Opt' && t.process_instance_id === processInstanceId)!;
  assert.equal(optTask.status, 'obsolete');

  const optToken = fake.store.tokens.find((t) => t.process_instance_id === processInstanceId && t.required === false)!;
  assert.equal(optToken.outcome, 'skipped');
  assert.equal(events(fake, 'token.skipped').length, 1);
  assert.equal(events(fake, 'token.joined').length, 1);
});

test('optional timer branch skipped at join cancels its pending job', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'f' }] },
      f: {
        id: 'f',
        type: 'fork',
        transitions: [
          { name: 'req', to: 'taskReq' },
          { name: 'opt', to: 'wait', required: false },
        ],
      },
      taskReq: { id: 'taskReq', type: 'task', name: 'Req', transitions: [{ name: 'done', to: 'j' }] },
      wait: {
        id: 'wait',
        type: 'timer',
        timer: { dueAtVariable: 'deadline', transition: 'fire' },
        transitions: [{ name: 'fire', to: 'j' }],
      },
      j: { id: 'j', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  await engine.startProcess({
    definitionKey: 'g',
    startedBy: 'x',
    variables: { deadline: '2099-01-01T00:00:00.000Z' },
  });

  // The optional timer branch scheduled a pending timer job.
  const timerJob = fake.store.jobs.find((j) => j.type === 'timer')!;
  assert.equal(timerJob.status, 'pending');

  const reqTask = fake.store.tasks.find((t) => t.name === 'Req')!;
  await engine.completeTask({ taskId: reqTask.id, userId: 'w', transitionName: 'done' });

  // The join skipped the optional branch; its timer job must no longer be pending.
  assert.equal(timerJob.status, 'cancelled');
  assert.equal(events(fake, 'job.cancelled').length, 1);
});

// ---------------------------------------------------------------------------
// Story 9 — join event payload
// ---------------------------------------------------------------------------

test('join event records participating branch ids and the result token', async () => {
  const graph = {
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
      taskA: { id: 'taskA', type: 'task', name: 'A', transitions: [{ name: 'done', to: 'j' }] },
      taskB: { id: 'taskB', type: 'task', name: 'B', transitions: [{ name: 'done', to: 'j' }] },
      j: { id: 'j', type: 'join', transitions: [{ name: 'go', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'g', startedBy: 'x' });
  const tasks = fake.store.tasks.filter((t) => t.process_instance_id === processInstanceId);
  for (const t of tasks) await engine.completeTask({ taskId: t.id, userId: 'w', transitionName: 'done' });

  const joinEvent = events(fake, 'token.joined')[0];
  assert.equal(joinEvent.node_id, 'j');
  assert.equal(joinEvent.data.branches.length, 2);
  assert.ok(joinEvent.data.resultTokenId);
});

// ---------------------------------------------------------------------------
// Story 10/11/12/13 — timers
// ---------------------------------------------------------------------------

test('timer node schedules a job, firing resumes the token', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TIMER_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const past = new Date(Date.now() - 5000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 't',
    startedBy: 'x',
    variables: { deadline: past },
  });

  let pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'active', 'process waits at the timer');

  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;
  assert.equal(job.type, 'timer');
  assert.equal(job.status, 'pending');
  assert.equal(events(fake, 'timer.scheduled').length, 1);

  const claimed = await engine.claimJobs('worker', 10);
  assert.equal(claimed.length, 1);
  assert.equal(claimed[0].id, job.id);

  await engine.fireTimerJob({ jobId: job.id, workerId: 'worker' });

  pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');
  assert.equal(pi!.outcome, 'completed');
  assert.equal(events(fake, 'timer.fired').length, 1);
});

test('timer firing is idempotent (no double transition)', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TIMER_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const past = new Date(Date.now() - 5000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 't',
    startedBy: 'x',
    variables: { deadline: past },
  });
  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;
  await engine.claimJobs('worker', 10);
  await engine.fireTimerJob({ jobId: job.id, workerId: 'worker' });

  await assert.rejects(
    engine.fireTimerJob({ jobId: job.id, workerId: 'worker' }),
    (err: any) => err instanceof WorkflowConflictError && err.code === 'TIMER_ALREADY_FIRED',
  );
});

test('timer cancel and reschedule', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('t', 1, TIMER_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  const future = new Date(Date.now() + 60000).toISOString();
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 't',
    startedBy: 'x',
    variables: { deadline: future },
  });
  const job = fake.store.jobs.find((j) => j.process_instance_id === processInstanceId)!;

  const newDue = new Date(Date.now() + 120000);
  await engine.rescheduleTimerJob({ jobId: job.id, newDueAt: newDue, actor: 'x' });
  assert.equal(events(fake, 'timer.rescheduled').length, 1);
  assert.equal(fake.store.jobs.find((j) => j.id === job.id)!.due_at.getTime(), newDue.getTime());

  await engine.cancelTimerJob({ jobId: job.id, actor: 'x' });
  assert.equal(fake.store.jobs.find((j) => j.id === job.id)!.status, 'cancelled');
  assert.equal(events(fake, 'timer.cancelled').length, 1);

  await engine.cancelTimerJob({ jobId: job.id, actor: 'x' }); // idempotent
});

// ---------------------------------------------------------------------------
// Story 14/15/16 — application command hook, commandId, result handling
// ---------------------------------------------------------------------------

test('command node invokes the application port with a stable commandId', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('c', 1, COMMAND_GRAPH);
  const app = makeApp();
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'c', startedBy: 'x' });

  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.status, 'completed');

  assert.equal(app.calls.length, 1);
  assert.equal(app.calls[0].commandType, 'c.do');
  const expectedCommandId = createHash('sha256').update(`${processInstanceId}:cmd:1`).digest('hex');
  assert.equal(app.calls[0].commandId, expectedCommandId);

  const record = fake.store.processCommands.find((r) => r.process_instance_id === processInstanceId)!;
  assert.equal(record.command_id, expectedCommandId);
  assert.equal(record.outcome, 'success');
  assert.equal(events(fake, 'command.requested').length, 1);
  assert.equal(events(fake, 'command.completed').length, 1);
});

test('ENG-FORGE-V10: repair revisit gets a new command identity while each visit stays deterministic', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
      cmd: {
        id: 'cmd',
        type: 'command',
        commandType: 'forge.publish_candidate',
        transition: 'done',
        transitions: [{ name: 'done', to: 'repair' }],
      },
      repair: {
        id: 'repair',
        type: 'task',
        name: 'Repair',
        transitions: [{ name: 'retry', to: 'cmd' }],
      },
    },
  };
  const fake = new FakeSql();
  fake.seedDefinition('visit', 1, graph);
  const app = makeApp();
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'visit',
    startedBy: 'x',
  });

  const firstRepair = fake.store.tasks.find(
    (task) => task.process_instance_id === processInstanceId && task.name === 'Repair',
  )!;
  await engine.completeTask({ taskId: firstRepair.id, userId: 'u', transitionName: 'retry' });

  assert.equal(app.calls.length, 2);
  assert.notEqual(app.calls[0].commandId, app.calls[1].commandId);
  assert.equal(
    app.calls[0].commandId,
    createHash('sha256').update(`${processInstanceId}:cmd:1`).digest('hex'),
  );
  assert.equal(
    app.calls[1].commandId,
    createHash('sha256').update(`${processInstanceId}:cmd:2`).digest('hex'),
  );
  assert.deepEqual(
    fake.store.processCommands.map((record) => record.visit_sequence),
    [1, 2],
  );
});

test('command conflict outcome terminates the process with conflict', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('c', 1, COMMAND_GRAPH);
  const app = makeApp({ executeCommand: async (req) => ({ commandId: req.commandId, outcome: 'conflict' }) });
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });

  const { processInstanceId } = await engine.startProcess({ definitionKey: 'c', startedBy: 'x' });
  const pi = await engine.getProcessInstance(processInstanceId);
  assert.equal(pi!.outcome, 'conflict');
  assert.equal(events(fake, 'command.failed').length, 1);
});

test('command failure outcomes terminate the process with failed', async () => {
  for (const outcome of ['validation_failure', 'not_found', 'unauthorized', 'precondition_failure']) {
    const fake = new FakeSql();
    fake.seedDefinition('c', 1, COMMAND_GRAPH);
    const app = makeApp({ executeCommand: async (req) => ({ commandId: req.commandId, outcome: outcome as any }) });
    const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });

    const { processInstanceId } = await engine.startProcess({ definitionKey: 'c', startedBy: 'x' });
    const pi = await engine.getProcessInstance(processInstanceId);
    assert.equal(pi!.outcome, 'failed', `outcome ${outcome} should terminate as failed`);
  }
});

test('command node without an application port raises a deterministic error', async () => {
  const fake = new FakeSql();
  fake.seedDefinition('c', 1, COMMAND_GRAPH);
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator });

  await assert.rejects(
    engine.startProcess({ definitionKey: 'c', startedBy: 'x' }),
    (err: any) => err.code === 'MISSING_APPLICATION_PORT',
  );
});

// ---------------------------------------------------------------------------
// Story 17 — fact refresh hook
// ---------------------------------------------------------------------------

test('decision refreshes application facts before evaluating', async () => {
  const graph = {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'd' }] },
      d: {
        id: 'd',
        type: 'decision',
        decisions: [{ condition: "stage == 'ready'", transition: 'ready' }],
        transitions: [
          { name: 'other', to: 'endOther' },
          { name: 'ready', to: 'endReady' },
        ],
      },
      endOther: { id: 'endOther', type: 'end' },
      endReady: { id: 'endReady', type: 'end' },
    },
  };

  const fake = new FakeSql();
  fake.seedDefinition('g', 1, graph);
  const app = makeApp({ readFacts: async () => ({ stage: 'ready' }) });
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app });

  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'g',
    startedBy: 'x',
    subject: { subjectType: 'deal', subjectId: 'd1' },
  });

  assert.equal(app.factCalls.length, 1);
  const tokens = fake.store.tokens.filter((t) => t.process_instance_id === processInstanceId);
  assert.equal(tokens[tokens.length - 1].node_id, 'endReady');
});
