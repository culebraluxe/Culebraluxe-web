import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { WorkflowConflictError } from '../../workflow_engine/lib/workflow/errors'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator, makeApp } from '../../workflow_engine/tests/fixtures'
import {
  transactionCloseV1Graph,
  TRANSACTION_CLOSE_V1_KEY,
  TRANSACTION_CLOSE_V1_VERSION,
} from '../definitions/transaction-close-v1'

type AnyRow = Record<string, any>

function setup(app = makeApp()) {
  const fake = new FakeSql()
  fake.seedDefinition(
    TRANSACTION_CLOSE_V1_KEY,
    TRANSACTION_CLOSE_V1_VERSION,
    transactionCloseV1Graph,
  )
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app })
  return { fake, engine, app }
}

function taskByName(fake: FakeSql, name: string): AnyRow {
  const list = fake.store.tasks.filter((r) => r.name === name)
  const t = list[list.length - 1]
  assert.ok(t, `expected task "${name}" to exist`)
  return t!
}

async function startAndReachMilestones(
  engine: WorkflowEngine,
  fake: FakeSql,
  financingApplicable: boolean | null,
): Promise<string> {
  const { processInstanceId } = await engine.startProcess({
    definitionKey: TRANSACTION_CLOSE_V1_KEY,
    startedBy: 'broker',
    variables: { financingApplicable },
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })

  await engine.completeTask({
    taskId: taskByName(fake, 'Prepare Contract').id,
    userId: 'broker',
    transitionName: 'ready',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Execute Contract').id,
    userId: 'broker',
    transitionName: 'executed',
  })
  return processInstanceId
}

async function completeMilestones(
  engine: WorkflowEngine,
  fake: FakeSql,
  names: string[],
) {
  for (const name of names) {
    await engine.completeTask({
      taskId: taskByName(fake, name).id,
      userId: 'sme',
      transitionName: 'done',
    })
  }
}

async function closeOut(engine: WorkflowEngine, fake: FakeSql) {
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'attorney',
    transitionName: 'closed',
  })
}

test('A. cash transaction happy path', async () => {
  const { fake, engine, app } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, false)

  await completeMilestones(engine, fake, ['Inspection', 'Title'])
  await closeOut(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
  assert.equal(app.calls.length, 2)
})

test('B. financed transaction happy path (all four milestones required)', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, true)

  await completeMilestones(engine, fake, [
    'Inspection',
    'Title',
    'Appraisal',
    'Financing',
  ])
  await closeOut(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
  const appraisalTask = fake.store.tasks.find((t) => t.name === 'Appraisal')!
  assert.equal(appraisalTask.status, 'completed')
})

test('C. cash transaction does not spawn appraisal/financing milestones', async () => {
  const { fake, engine } = setup()
  await startAndReachMilestones(engine, fake, false)

  assert.equal(fake.store.tokens.find((t) => t.node_id === 'appraisal'), undefined)
  assert.equal(fake.store.tokens.find((t) => t.node_id === 'financing'), undefined)
})

test('D. inspection blocker then resolved', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, true)

  await engine.completeTask({
    taskId: taskByName(fake, 'Inspection').id,
    userId: 'inspector',
    transitionName: 'blocker',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Inspection Blocker').id,
    userId: 'inspector',
    transitionName: 'resolved',
  })
  await completeMilestones(engine, fake, [
    'Inspection',
    'Title',
    'Appraisal',
    'Financing',
  ])
  await closeOut(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('E. financing failure terminates the process', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, true)

  await engine.completeTask({
    taskId: taskByName(fake, 'Financing').id,
    userId: 'lender',
    transitionName: 'fail',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

test('F. title blocker escalated to failure', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, true)

  await engine.completeTask({
    taskId: taskByName(fake, 'Title').id,
    userId: 'title',
    transitionName: 'blocker',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Title Blocker').id,
    userId: 'title',
    transitionName: 'escalate',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

test('G. cancellation before contract', async () => {
  const { fake, engine } = setup()
  const { processInstanceId } = await engine.startProcess({
    definitionKey: TRANSACTION_CLOSE_V1_KEY,
    startedBy: 'broker',
    variables: { financingApplicable: false },
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })

  await engine.cancelProcess({ processInstanceId, actor: 'broker' })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'cancelled')
})

test('H. cancellation under contract', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, false)
  await engine.cancelProcess({ processInstanceId, actor: 'broker' })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'cancelled')
})

test('I. overdue deadline timer escalates', async () => {
  const fake = new FakeSql()
  fake.seedDefinition('deadline-escalation', 1, {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'wait' }] },
      wait: {
        id: 'wait',
        type: 'timer',
        timer: { dueAtVariable: 'deadline', transition: 'fire' },
        transitions: [{ name: 'fire', to: 'escalate' }],
      },
      escalate: { id: 'escalate', type: 'task', name: 'Escalate', transitions: [{ name: 'done', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  })
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator })

  const past = new Date(Date.now() - 5000).toISOString()
  await engine.startProcess({
    definitionKey: 'deadline-escalation',
    startedBy: 'broker',
    variables: { deadline: past },
  })

  const claimed = await engine.claimJobs('worker', 10)
  assert.equal(claimed.length, 1)
  await engine.fireTimerJob({ jobId: claimed[0].id, workerId: 'worker' })

  assert.ok(taskByName(fake, 'Escalate'))
})

test('J. command ids are stable and deduplicated', async () => {
  const { fake, engine, app } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, false)
  await completeMilestones(engine, fake, ['Inspection', 'Title'])
  await closeOut(engine, fake)

  const records = fake.store.processCommands.filter(
    (r) => r.process_instance_id === processInstanceId,
  )
  assert.equal(records.length, 2)
  for (const r of records) {
    const expected = createHash('sha256')
      .update(`${processInstanceId}:${r.node_id}`)
      .digest('hex')
    assert.equal(r.command_id, expected)
  }
  assert.equal(app.calls.length, 2)
})

test('L. application conflict terminates the process with conflict', async () => {
  const { fake, engine } = setup(
    makeApp({ executeCommand: async (req) => ({ commandId: req.commandId, outcome: 'conflict' as const }) }),
  )
  const { processInstanceId } = await engine.startProcess({
    definitionKey: TRANSACTION_CLOSE_V1_KEY,
    startedBy: 'broker',
    variables: { financingApplicable: false },
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Prepare Contract').id,
    userId: 'broker',
    transitionName: 'ready',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Execute Contract').id,
    userId: 'broker',
    transitionName: 'executed',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'conflict')
})

test('M. application command failure terminates the process as failed', async () => {
  const { fake, engine } = setup(
    makeApp({ executeCommand: async (req) => ({ commandId: req.commandId, outcome: 'validation_failure' as const }) }),
  )
  const { processInstanceId } = await engine.startProcess({
    definitionKey: TRANSACTION_CLOSE_V1_KEY,
    startedBy: 'broker',
    variables: { financingApplicable: false },
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Prepare Contract').id,
    userId: 'broker',
    transitionName: 'ready',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Execute Contract').id,
    userId: 'broker',
    transitionName: 'executed',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

test('N. ready-to-close is reached via the join', async () => {
  const { fake, engine } = setup()
  await startAndReachMilestones(engine, fake, false)
  await completeMilestones(engine, fake, ['Inspection', 'Title'])

  const joined = fake.store.processEvents.find((e) => e.event_type === 'token.joined')
  assert.ok(joined, 'join must release after required milestones')
  assert.ok(taskByName(fake, 'Closing'))
})

test('O. successful closing', async () => {
  const { fake, engine } = setup()
  const processInstanceId = await startAndReachMilestones(engine, fake, false)
  await completeMilestones(engine, fake, ['Inspection', 'Title'])
  await closeOut(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
  assert.equal(fake.store.processCommands.length, 2)
})

// ---------------------------------------------------------------------------
// Story 93 — unknown financing is not cash
// ---------------------------------------------------------------------------

test('financingApplicable null does NOT enter the cash fork', async () => {
  let state: boolean | null = null
  const { fake, engine } = setup(
    makeApp({ readFacts: async () => ({ financingApplicable: state }) }),
  )
  await startAndReachMilestones(engine, fake, null)

  assert.ok(taskByName(fake, 'Resolve Financing Applicability'))
  assert.equal(fake.store.tasks.find((t) => t.name === 'Inspection'), undefined)
})

test('financingApplicable null later resolved true enters the financed fork', async () => {
  let resolved: 'cash' | 'financed' | null = null
  const { fake, engine } = setup(
    makeApp({
      executeCommand: async (req) => {
        if (req.commandType === 'deal.set_financing_type') resolved = req.input.financingType
        return { commandId: req.commandId, outcome: 'success' as const }
      },
      readFacts: async () => ({
        financingApplicable: resolved === 'financed' ? true : resolved === 'cash' ? false : null,
      }),
    }),
  )
  await startAndReachMilestones(engine, fake, null)
  assert.ok(taskByName(fake, 'Resolve Financing Applicability'))

  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Financing Applicability').id,
    userId: 'broker',
    formData: { financingType: 'financed' },
    transitionName: 'resolved',
  })

  assert.ok(taskByName(fake, 'Appraisal'), 'financed fork spawns appraisal')
  assert.ok(taskByName(fake, 'Financing'))
})

test('financingApplicable null later resolved false enters the cash fork', async () => {
  let resolved: 'cash' | 'financed' | null = null
  const { fake, engine } = setup(
    makeApp({
      executeCommand: async (req) => {
        if (req.commandType === 'deal.set_financing_type') resolved = req.input.financingType
        return { commandId: req.commandId, outcome: 'success' as const }
      },
      readFacts: async () => ({
        financingApplicable: resolved === 'financed' ? true : resolved === 'cash' ? false : null,
      }),
    }),
  )
  await startAndReachMilestones(engine, fake, null)

  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Financing Applicability').id,
    userId: 'broker',
    formData: { financingType: 'cash' },
    transitionName: 'resolved',
  })

  assert.ok(taskByName(fake, 'Inspection'), 'cash fork spawns inspection')
  assert.equal(fake.store.tasks.find((t) => t.name === 'Financing'), undefined)
})

test('duplicate task completion is rejected (retry safety)', async () => {
  const { fake, engine } = setup()
  await startAndReachMilestones(engine, fake, false)
  const inspection = taskByName(fake, 'Inspection')
  await engine.completeTask({
    taskId: inspection.id,
    userId: 'sme',
    transitionName: 'done',
  })
  await assert.rejects(
    engine.completeTask({ taskId: inspection.id, userId: 'sme', transitionName: 'done' }),
    /cannot be completed/,
  )
})

test('stale conflict guard still holds in the brokerage model', async () => {
  const { fake, engine } = setup()
  await startAndReachMilestones(engine, fake, false)
  const inspection = taskByName(fake, 'Inspection')
  const tokenId = fake.store.tasks.find((t) => t.id === inspection.id)!.token_id
  const token = await engine.getToken(tokenId)
  await assert.rejects(
    (engine as any)._moveToken(fake.sql, { ...token, version: 999 }, 'x', 't', 'x'),
    (err: any) => err instanceof WorkflowConflictError,
  )
})
