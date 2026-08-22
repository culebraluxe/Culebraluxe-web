import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { WorkflowConflictError } from '../../workflow_engine/lib/workflow/errors'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator, makeApp } from '../../workflow_engine/tests/fixtures'
import {
  parseReSupermodel,
  RE_SUPERMODEL_KEY,
  RE_SUPERMODEL_VERSION,
} from '../definitions/re-supermodel'

// ---------------------------------------------------------------------------
// Story 126 — XML-driven scenario tests.
//
// Every scenario runs the RE_supermodel ProcessGraph derived from the XML
// file (not a hand-written TypeScript graph), through the real WorkflowEngine
// against the in-memory FakeSql. No database, no packages.
// ---------------------------------------------------------------------------

type AnyRow = Record<string, any>

function makeReApp(initial: Record<string, any>) {
  let facts: Record<string, any> = { ...initial }
  const calls: any[] = []
  return {
    calls,
    facts: () => ({ ...facts }),
    setFact: (k: string, v: any) => {
      facts = { ...facts, [k]: v }
    },
    app: {
      async executeCommand(req: any) {
        calls.push(req)
        if (req.commandType === 'deal.set_closing_date') {
          facts = {
            ...facts,
            closingDateScheduled: true,
            closingDate: req.input.closingDate ?? '2030-01-01',
          }
        }
        return { commandId: req.commandId, outcome: 'success' as const }
      },
      async readFacts() {
        return { ...facts }
      },
    },
  }
}

function setup(initialFacts: Record<string, any> = {}) {
  const parsed = parseReSupermodel()
  const fake = new FakeSql()
  fake.seedDefinition(RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION, parsed.graph)
  const re = makeReApp(initialFacts)
  const engine = new WorkflowEngine(fake.sql, {
    evaluate: stubEvaluator,
    app: re.app as any,
  })
  return { fake, engine, re, graph: parsed.graph }
}

/** Facts for a clean cash transaction that should take the short path. */
function simpleCashFacts(overrides: Record<string, any> = {}) {
  return {
    financingApplicable: false,
    appraisalApplicable: false,
    inspectionApplicable: false,
    insuranceApplicable: false,
    requiresSurvey: false,
    requiresHoaClearance: false,
    requiresRegistryFollowup: false,
    closingConfirmationRequired: false,
    closingDateScheduled: false,
    ...overrides,
  }
}

function taskByName(fake: FakeSql, name: string): AnyRow {
  const list = fake.store.tasks.filter((t) => t.name === name)
  const t = list[list.length - 1]
  assert.ok(t, `expected task "${name}" to exist`)
  return t!
}

function hasTask(fake: FakeSql, name: string): boolean {
  return fake.store.tasks.some((t) => t.name === name)
}

async function startAndSignContract(engine: WorkflowEngine, fake: FakeSql): Promise<string> {
  const { processInstanceId } = await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'broker',
    variables: {},
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Contract / P&S Preparation').id,
    userId: 'broker',
    transitionName: 'prepared',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Contract / P&S Executed').id,
    userId: 'broker',
    transitionName: 'executed',
  })
  return processInstanceId
}

async function completeTasks(
  engine: WorkflowEngine,
  fake: FakeSql,
  names: string[],
  transition = 'done',
) {
  for (const name of names) {
    await engine.completeTask({
      taskId: taskByName(fake, name).id,
      userId: 'sme',
      transitionName: transition,
    })
  }
}

// ---------------------------------------------------------------------------
// A. simple cash clean transaction (short path through the SAME supermodel)
// ---------------------------------------------------------------------------
test('A. simple cash transaction takes the short path — no placeholder tracks', async () => {
  const { fake, engine, re } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  // The fork spawned only the four required tracks — no conditional tracks.
  assert.ok(hasTask(fake, 'Title / Legal'))
  assert.ok(hasTask(fake, 'Tax / Municipal Clearance'))
  assert.ok(hasTask(fake, 'Funds Ready'))
  assert.ok(hasTask(fake, 'Closing Documents'))
  assert.equal(hasTask(fake, 'Inspection'), false, 'cash simple path must not create Inspection')
  assert.equal(hasTask(fake, 'Financing'), false, 'cash simple path must not create Financing')
  assert.equal(hasTask(fake, 'Appraisal'), false, 'no appraisal branch unless applicable')
  assert.equal(hasTask(fake, 'Insurance'), false)
  assert.equal(hasTask(fake, 'Survey'), false)
  assert.equal(hasTask(fake, 'HOA / Condo Clearance'), false)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  // closingConfirmationRequired=false routes straight to ready-to-close.
  assert.equal(hasTask(fake, 'Confirm Closing Readiness'), false)

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })

  // requiresRegistryFollowup=false: no recording task; straight to terminal.
  assert.equal(hasTask(fake, 'Recording / Registry Follow-up'), false)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
  // mark_under_contract + mark_closed commands ran.
  assert.equal(re.calls.filter((c) => c.commandType === 'deal.set_stage_under_contract').length, 1)
  assert.equal(re.calls.filter((c) => c.commandType === 'deal.set_stage_closed').length, 1)
})

// ---------------------------------------------------------------------------
// B. financed transaction (all conditional tracks active and completed)
// ---------------------------------------------------------------------------
test('B. financed transaction completes with all applicable tracks', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      financingApplicable: true,
      appraisalApplicable: true,
      inspectionApplicable: true,
      insuranceApplicable: true,
      requiresSurvey: true,
      requiresHoaClearance: true,
      requiresRegistryFollowup: true,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Inspection'))
  assert.ok(hasTask(fake, 'Financing'))
  assert.ok(hasTask(fake, 'Appraisal'))
  assert.ok(hasTask(fake, 'Insurance'))
  assert.ok(hasTask(fake, 'Survey'))
  assert.ok(hasTask(fake, 'HOA / Condo Clearance'))

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Inspection',
    'Financing',
    'Appraisal',
    'Insurance',
    'Survey',
    'HOA / Condo Clearance',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  await engine.completeTask({
    taskId: taskByName(fake, 'Recording / Registry Follow-up').id,
    userId: 'sme',
    transitionName: 'done',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// C. cash + appraisal — appraisal is independent of financing (Story 123)
// ---------------------------------------------------------------------------
test('C. cash transaction with appraisalApplicable creates appraisal but no financing', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({ appraisalApplicable: true }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Appraisal'), 'cash deal with appraisalApplicable must appraise')
  assert.equal(hasTask(fake, 'Financing'), false, 'cash deal must not run financing')

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Appraisal',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// D. financed without appraisal — financing does not force appraisal (Story 123)
// ---------------------------------------------------------------------------
test('D. financed transaction with appraisalApplicable=false skips appraisal', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      financingApplicable: true,
      appraisalApplicable: false,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Financing'))
  assert.equal(hasTask(fake, 'Appraisal'), false, 'financing must not force an appraisal')

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// D2. null appraisalApplicable — handled explicitly, never silently skipped
// (CRM-19). The unresolved applicability surfaces a resolution task; resolving
// it to false skips deterministically only after the explicit resolution.
// ---------------------------------------------------------------------------
test('D2. unresolved appraisalApplicable surfaces an explicit resolution task and skips only after resolution', async () => {
  const { fake, engine, re } = setup(simpleCashFacts({ appraisalApplicable: null }))
  const processInstanceId = await startAndSignContract(engine, fake)

  // null must NOT silently skip: an explicit resolution task appears instead.
  assert.ok(hasTask(fake, 'Resolve Appraisal Applicability'), 'null applicability must surface the resolution task')
  assert.equal(hasTask(fake, 'Appraisal'), false, 'appraisal must not run while unresolved')

  // Completing "resolved" with the fact STILL null loops back to the decision,
  // which re-surfaces the same explicit task (blocker loop, never a skip).
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Appraisal Applicability').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.ok(hasTask(fake, 'Resolve Appraisal Applicability'), 'still unresolved: the resolution task must reappear')
  assert.equal(hasTask(fake, 'Appraisal'), false)

  // A human/application resolves the canonical fact to false.
  re.setFact('appraisalApplicable', false)
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Appraisal Applicability').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.equal(hasTask(fake, 'Appraisal'), false, 'resolved to false: appraisal skips deterministically')

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// D3. unresolved appraisalApplicable resolved to true — the branch executes
// (CRM-19). Proves the explicit-resolution path activates the appraisal branch.
// ---------------------------------------------------------------------------
test('D3. unresolved appraisalApplicable resolved to true runs the appraisal branch', async () => {
  const { fake, engine, re } = setup(simpleCashFacts({ appraisalApplicable: null }))
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Resolve Appraisal Applicability'), 'null applicability must surface the resolution task')

  // A human/application resolves the canonical fact to true (lender/buyer/seller request).
  re.setFact('appraisalApplicable', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Appraisal Applicability').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.ok(hasTask(fake, 'Appraisal'), 'resolved to true: the appraisal branch must execute')

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Appraisal',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// E. inspection issue resolved via the repair loop
// ---------------------------------------------------------------------------
test('E. inspection issue is resolved through the blocker loop', async () => {
  const { fake, engine } = setup(simpleCashFacts({ inspectionApplicable: true }))
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Inspection').id,
    userId: 'inspector',
    transitionName: 'issue',
  })
  assert.ok(hasTask(fake, 'Resolve Inspection Issue / Repairs'))
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Inspection Issue / Repairs').id,
    userId: 'inspector',
    transitionName: 'resolved',
  })
  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Inspection',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// F. title defect cured through the blocker loop
// ---------------------------------------------------------------------------
test('F. title defect is cured through the blocker loop', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Title / Legal').id,
    userId: 'title',
    transitionName: 'issue',
  })
  assert.ok(hasTask(fake, 'Resolve Title Defect'))
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Title Defect').id,
    userId: 'title',
    transitionName: 'resolved',
  })
  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// G. financing failure terminates the transaction
// ---------------------------------------------------------------------------
test('G. financing failure terminates the transaction', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({ financingApplicable: true }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Financing').id,
    userId: 'lender',
    transitionName: 'fail',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

// ---------------------------------------------------------------------------
// H. appraisal issue escalates to failure
// ---------------------------------------------------------------------------
test('H. unresolvable appraisal gap terminates the transaction', async () => {
  const { fake, engine } = setup(simpleCashFacts({ appraisalApplicable: true }))
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Appraisal').id,
    userId: 'appraiser',
    transitionName: 'issue',
  })
  assert.ok(hasTask(fake, 'Resolve Appraisal Gap'))
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Appraisal Gap').id,
    userId: 'appraiser',
    transitionName: 'escalate',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

// ---------------------------------------------------------------------------
// I. closing date extension — same instance continues, timer reschedules
// ---------------------------------------------------------------------------
test('I. closing date extension reschedules the timer on the SAME instance', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      closingDateScheduled: true,
      closingDate: '2020-01-01T00:00:00.000Z', // past => timer is due
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  // Reach ready_to_close (all required tracks done; readiness verified).
  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  // The deadline monitor scheduled a timer for the (past) closing date.
  const claimed = await engine.claimJobs('worker', 10)
  assert.equal(claimed.length, 1, 'the closing-date timer should be due')

  await engine.fireTimerJob({ jobId: claimed[0].id, workerId: 'worker' })
  assert.ok(hasTask(fake, 'Closing Date Escalation'), 'passed closing date escalates')

  // Extend: amend the closing date — the same instance continues.
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing Date Escalation').id,
    userId: 'broker',
    formData: { closingDate: '2030-01-01T00:00:00.000Z' },
    transitionName: 'extend',
  })

  assert.ok(
    re.calls.some((c) => c.commandType === 'deal.set_closing_date'),
    'deal.set_closing_date must be issued on amendment',
  )

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.status, 'active', 'the instance must not restart when the date changes')

  // A fresh pending timer job exists for the amended date.
  const pending = fake.store.jobs.filter(
    (j) => j.process_instance_id === processInstanceId && j.status === 'pending',
  )
  assert.equal(pending.length, 1)
  assert.equal(new Date(pending[0].due_at).toISOString(), '2030-01-01T00:00:00.000Z')
})

// ---------------------------------------------------------------------------
// J. tax / CRIM blocker resolved
// ---------------------------------------------------------------------------
test('J. tax / CRIM blocker is resolved', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Tax / Municipal Clearance').id,
    userId: 'broker',
    transitionName: 'issue',
  })
  assert.ok(hasTask(fake, 'Resolve Tax / CRIM Blocker'))
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Tax / CRIM Blocker').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// K. HOA / survey / insurance branches skipped when not applicable
// ---------------------------------------------------------------------------
test('K. HOA / survey / insurance branches are skipped when not applicable', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.equal(hasTask(fake, 'Insurance'), false)
  assert.equal(hasTask(fake, 'Survey'), false)
  assert.equal(hasTask(fake, 'HOA / Condo Clearance'), false)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// L. post-closing registry / recording follow-up (Story 125)
// ---------------------------------------------------------------------------
test('L1. post-closing recording follow-up runs after deal.stage is closed', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({ requiresRegistryFollowup: true }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })

  // deal.stage closed command fired BEFORE post-closing recording work.
  assert.ok(
    re.calls.some((c) => c.commandType === 'deal.set_stage_closed'),
    'deal.stage must be set to closed before recording follow-up',
  )
  assert.ok(hasTask(fake, 'Recording / Registry Follow-up'))

  await engine.completeTask({
    taskId: taskByName(fake, 'Recording / Registry Follow-up').id,
    userId: 'sme',
    transitionName: 'done',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('L2. post-closing recording is skipped when not applicable', async () => {
  const { fake, engine } = setup(simpleCashFacts({ requiresRegistryFollowup: false }))
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })

  assert.equal(hasTask(fake, 'Recording / Registry Follow-up'), false)
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// M. cancellation / failed transaction
// ---------------------------------------------------------------------------
test('M. cancellation from P&S preparation terminates with cancelled', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const { processInstanceId } = await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'broker',
    variables: {},
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })

  await engine.completeTask({
    taskId: taskByName(fake, 'Contract / P&S Preparation').id,
    userId: 'broker',
    transitionName: 'cancel',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'cancelled')
})

// ---------------------------------------------------------------------------
// N. a new historical attempt remains possible after a terminal prior instance
// ---------------------------------------------------------------------------
test('N. a new transaction attempt is possible after a terminal prior instance', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const first = await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'broker',
    variables: {},
    subject: { subjectType: 'deal', subjectId: 'deal-history' },
  })
  await engine.cancelProcess({ processInstanceId: first.processInstanceId, actor: 'broker' })

  const firstPi = await engine.getProcessInstance(first.processInstanceId)
  assert.equal(firstPi!.outcome, 'cancelled')

  const second = await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'broker',
    variables: {},
    subject: { subjectType: 'deal', subjectId: 'deal-history' },
  })
  assert.notEqual(second.processInstanceId, first.processInstanceId)

  const secondPi = await engine.getProcessInstance(second.processInstanceId)
  assert.equal(secondPi!.status, 'active')
})

// ---------------------------------------------------------------------------
// Story 124 / 136 — closing readiness: structural eligibility + optional
// human confirmation. The confirmation cannot override blockers (it follows
// the join) and is only added when closingConfirmationRequired is true.
// ---------------------------------------------------------------------------
test('closing confirmation is optional and added only when required', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({ closingConfirmationRequired: true }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  // Fork-joined; the confirmation gate now requires the human step.
  assert.ok(hasTask(fake, 'Confirm Closing Readiness'), 'confirmation must be requested')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start before confirmation')

  await engine.completeTask({
    taskId: taskByName(fake, 'Confirm Closing Readiness').id,
    userId: 'broker',
    transitionName: 'resolved',
  })

  // Confirmation releases straight to ready-to-close, then closing.
  assert.ok(hasTask(fake, 'Closing'), 'closing task must appear after confirmation')
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

// ---------------------------------------------------------------------------
// Story 140 — generic engine/application-port behaviors preserved from the
// retired transaction-close-v1 scenario suite, now driven by the XML model.
// ---------------------------------------------------------------------------

function setupCustomApp(app: any) {
  const parsed = parseReSupermodel()
  const fake = new FakeSql()
  fake.seedDefinition(RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION, parsed.graph)
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app })
  return { fake, engine }
}

test('command ids are stable and deterministic (sha256 of instance:node)', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  const records = fake.store.processCommands.filter(
    (r) => r.process_instance_id === processInstanceId,
  )
  assert.equal(records.length, 1) // mark_under_contract
  const expected = createHash('sha256')
    .update(`${processInstanceId}:mark_under_contract`)
    .digest('hex')
  assert.equal(records[0].command_id, expected)
})

test('application conflict terminates the process with conflict', async () => {
  const app = makeApp({
    executeCommand: async (req: any) => ({
      commandId: req.commandId,
      outcome: 'conflict' as const,
    }),
  })
  const { fake, engine } = setupCustomApp(app)
  const processInstanceId = await startAndSignContract(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'conflict')
})

test('application command failure terminates the process as failed', async () => {
  const app = makeApp({
    executeCommand: async (req: any) => ({
      commandId: req.commandId,
      outcome: 'validation_failure' as const,
    }),
  })
  const { fake, engine } = setupCustomApp(app)
  const processInstanceId = await startAndSignContract(engine, fake)

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'failed')
})

test('duplicate task completion is rejected (retry safety)', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  await startAndSignContract(engine, fake)
  const title = taskByName(fake, 'Title / Legal')
  await engine.completeTask({
    taskId: title.id,
    userId: 'sme',
    transitionName: 'done',
  })
  await assert.rejects(
    engine.completeTask({ taskId: title.id, userId: 'sme', transitionName: 'done' }),
    /cannot be completed/,
  )
})

test('stale token move raises deterministic conflict', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  await startAndSignContract(engine, fake)
  const title = taskByName(fake, 'Title / Legal')
  const tokenId = fake.store.tasks.find((t) => t.id === title.id)!.token_id
  const token = await engine.getToken(tokenId)
  await assert.rejects(
    (engine as any)._moveToken(fake.sql, { ...token, version: 999 }, 'x', 't', 'x'),
    (err: any) => err instanceof WorkflowConflictError,
  )
})

// ---------------------------------------------------------------------------
// Story 141 — funds-not-ready complexity path (same RE_supermodel XML).
// ---------------------------------------------------------------------------
test('funds not ready is resolved through the blocker loop', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  await engine.completeTask({
    taskId: taskByName(fake, 'Funds Ready').id,
    userId: 'buyer',
    transitionName: 'issue',
  })
  assert.ok(hasTask(fake, 'Resolve Funds'))
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Funds').id,
    userId: 'buyer',
    transitionName: 'resolved',
  })
  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})
