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
        // CRM-22 — deadline amendment commands refresh the canonical facts so
        // the re-armed timer node reads the amended date.
        if (req.commandType === 'deal.set_inspection_deadline') {
          facts = {
            ...facts,
            inspectionDeadlineScheduled: true,
            inspectionDeadline: req.input.inspectionDeadline ?? '2030-01-01',
          }
        }
        if (req.commandType === 'deal.set_financing_deadline') {
          facts = {
            ...facts,
            financingDeadlineScheduled: true,
            financingDeadline: req.input.financingDeadline ?? '2030-01-01',
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
    // CRM-21: the canonical closing-document packet is complete and signed
    // (derived fact). Scenarios that need to test the not-ready path override
    // this to false.
    closingDocumentsReady: true,
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

/** True when a task with `name` currently exists and is still actionable. */
function hasOpenTask(fake: FakeSql, name: string): boolean {
  return fake.store.tasks.some(
    (t) => t.name === name && ['ready', 'reserved', 'in_progress'].includes(t.status),
  )
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
      // CRM-20: a financed deal must clear lender clear-to-close before
      // closing readiness can succeed.
      lenderClearToClose: true,
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
      // CRM-20: financed deals clear lender clear-to-close before readiness.
      lenderClearToClose: true,
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
// CRM-20 — lender clear-to-close (canonical deal.lender_clear_to_close fact).
// Financed deals must clear the lender gate before closing readiness can
// succeed; cash/non-financed deals are unaffected. True/false/unknown each
// produce the documented workflow path.
// ---------------------------------------------------------------------------
test('CRM-20: financed deal with lender clear-to-close true reaches closing readiness', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: true,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  // Cleared: no lender-clearance tasks surface; readiness proceeds.
  assert.equal(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), false)
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cleared: closing must be reachable')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-20: financed deal without lender clear-to-close (false) cannot be closing-ready', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: false,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  // The deal must NOT appear closing-ready while the lender has not cleared.
  assert.ok(hasOpenTask(fake, 'Lender Clearance Pending'), 'not cleared: pending task must surface')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start before lender clearance')

  // Re-attempt with the fact still false: the pending task reappears (a
  // blocker loop, never a silent pass).
  await engine.completeTask({
    taskId: taskByName(fake, 'Lender Clearance Pending').id,
    userId: 'lender',
    transitionName: 'resolved',
  })
  assert.ok(hasOpenTask(fake, 'Lender Clearance Pending'), 'still not cleared: pending must reappear')
  assert.equal(hasTask(fake, 'Closing'), false)

  // The lender clears (application command deal.set_lender_clear_to_close -> true).
  re.setFact('lenderClearToClose', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Lender Clearance Pending').id,
    userId: 'lender',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cleared: closing becomes reachable')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-20: unresolved lender clear-to-close (null) surfaces an explicit resolution task, never a silent pass', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: null,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  // null must NOT silently pass: an explicit resolution task appears instead.
  assert.ok(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), 'null must surface the resolution task')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start while lender clearance is unresolved')

  // Completing "resolved" while the fact is STILL null loops back to the same
  // explicit task (blocker loop, never a silent pass).
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Lender Clear-to-Close').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.ok(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), 'still unresolved: the resolution task must reappear')
  assert.equal(hasTask(fake, 'Closing'), false)

  // A human/application records lender clearance (true).
  re.setFact('lenderClearToClose', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Lender Clear-to-Close').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cleared: closing becomes reachable')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-20: resolving unresolved lender clearance to false lands in the pending state, not readiness', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: null,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  assert.ok(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), 'null must surface the resolution task')

  // A human/application records that the lender has NOT cleared (false).
  re.setFact('lenderClearToClose', false)
  await engine.completeTask({
    taskId: taskByName(fake, 'Resolve Lender Clear-to-Close').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.ok(hasOpenTask(fake, 'Lender Clearance Pending'), 'not cleared: the pending state must take over')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start while the lender has not cleared')

  // The lender eventually clears -> readiness proceeds.
  re.setFact('lenderClearToClose', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Lender Clearance Pending').id,
    userId: 'lender',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cleared: closing becomes reachable')
})

test('CRM-20: cash deals are unaffected by lender clear-to-close (fact null)', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({ lenderClearToClose: null }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  // Cash: no financing branch and no lender-clearance gate — readiness proceeds
  // straight through even though the canonical fact is null (never recorded).
  assert.equal(hasTask(fake, 'Financing'), false)
  assert.equal(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), false, 'cash deals never surface lender-clearance tasks')
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cash deal reaches closing without lender clearance')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-20: financed deal with lender clearance still honors the optional closing confirmation', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: true,
      closingConfirmationRequired: true,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  // Cleared, but the optional brokerage confirmation still applies.
  assert.equal(hasOpenTask(fake, 'Resolve Lender Clear-to-Close'), false)
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasOpenTask(fake, 'Confirm Closing Readiness'), 'confirmation must still be requested after lender clearance')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start before confirmation')

  await engine.completeTask({
    taskId: taskByName(fake, 'Confirm Closing Readiness').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
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
// CRM-21 — closing-document readiness (derived fact, consumed by the
// closing_documents_gate before closing readiness). The fact is true only when
// the canonical closing-document packet is complete AND signed/final; false
// blocks readiness with an explicit pending task, never a silent pass.
// ---------------------------------------------------------------------------
test('CRM-21: missing/incomplete closing documents (fact false) block closing readiness', async () => {
  const { fake, engine, re } = setup(simpleCashFacts({ closingDocumentsReady: false }))
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  // The canonical closing-document packet is not ready: the pending task must
  // surface and closing must NOT be reachable — the bare "Closing Documents"
  // human task alone cannot make the deal closing-ready.
  assert.ok(hasOpenTask(fake, 'Closing Documents Pending'), 'not ready: pending must surface')
  assert.equal(hasTask(fake, 'Closing'), false, 'closing must not start before the packet is signed/final')

  // Completing "resolved" while the fact is STILL false loops back to the same
  // pending task (blocker loop, never a silent pass).
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing Documents Pending').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.ok(hasOpenTask(fake, 'Closing Documents Pending'), 'still not ready: pending must reappear')
  assert.equal(hasTask(fake, 'Closing'), false)

  // The closing documents become complete and signed (packet + DOC-01 signed
  // lineage -> derived fact true); the gate re-evaluates on resolved.
  re.setFact('closingDocumentsReady', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing Documents Pending').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Closing Documents Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'ready: closing becomes reachable')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-21: a complete signed closing packet passes the gate for cash deals', async () => {
  const { fake, engine } = setup(simpleCashFacts()) // closingDocumentsReady: true
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
  ])

  assert.equal(hasOpenTask(fake, 'Closing Documents Pending'), false, 'ready: no pending task')
  assert.ok(hasTask(fake, 'Closing'), 'ready: closing reachable')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-21: closing documents gate runs BEFORE the lender gate (financed deals)', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      financingApplicable: true,
      lenderClearToClose: false,
      closingDocumentsReady: false,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  await completeTasks(engine, fake, [
    'Title / Legal',
    'Tax / Municipal Clearance',
    'Funds Ready',
    'Closing Documents',
    'Financing',
  ])

  // Both facts are not ready; the closing-document gate is evaluated first, so
  // the documents pending task surfaces and the lender gate is not reached yet.
  assert.ok(hasOpenTask(fake, 'Closing Documents Pending'), 'docs gate precedes the lender gate')
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.equal(hasTask(fake, 'Closing'), false)

  // Documents become ready; re-evaluation now surfaces the lender pending task.
  re.setFact('closingDocumentsReady', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Closing Documents Pending').id,
    userId: 'broker',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Closing Documents Pending'), false)
  assert.ok(hasOpenTask(fake, 'Lender Clearance Pending'), 'lender gate now surfaces')
  assert.equal(hasTask(fake, 'Closing'), false)

  // Lender clears; readiness proceeds to closing.
  re.setFact('lenderClearToClose', true)
  await engine.completeTask({
    taskId: taskByName(fake, 'Lender Clearance Pending').id,
    userId: 'lender',
    transitionName: 'resolved',
  })
  assert.equal(hasOpenTask(fake, 'Lender Clearance Pending'), false)
  assert.ok(hasTask(fake, 'Closing'), 'cleared: closing becomes reachable')

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

test('command ids are stable and deterministic (sha256 of instance:node:visit)', async () => {
  const { fake, engine } = setup(simpleCashFacts())
  const processInstanceId = await startAndSignContract(engine, fake)

  const records = fake.store.processCommands.filter(
    (r) => r.process_instance_id === processInstanceId,
  )
  assert.equal(records.length, 1) // mark_under_contract
  const expected = createHash('sha256')
    .update(`${processInstanceId}:mark_under_contract:1`)
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

// ---------------------------------------------------------------------------
// CRM-22 — transaction deadline fact sources.
//
// The inspection / financing deadline monitors are OPTIONAL fork branches
// gated by applicability + scheduled decisions. Each timer reads its due date
// from a canonical fact (due-at-variable) — a deadline never exists without
// an application source. On passage the branch escalates to a human task;
// amending issues the canonical deadline command and the SAME instance
// continues (timer re-armed). When the milestone completes, the join skips
// the still-active optional monitor and cancels its pending timer.
// ---------------------------------------------------------------------------

/** The required transaction tracks for the short closing path (CRM-22). */
const REQUIRED_TRACKS = [
  'Title / Legal',
  'Tax / Municipal Clearance',
  'Funds Ready',
  'Closing Documents',
]

function pendingTimers(fake: FakeSql, processInstanceId: string) {
  return fake.store.jobs.filter(
    (j) => j.process_instance_id === processInstanceId && j.status === 'pending',
  )
}

test('M1. inspection deadline monitor escalates on passage; amendment re-arms the SAME instance', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      inspectionApplicable: true,
      inspectionDeadlineScheduled: true,
      // Past date => the inspection deadline timer is due immediately.
      inspectionDeadline: '2020-01-01T00:00:00.000Z',
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Inspection'), 'inspection track is active')
  const due = await engine.claimJobs('worker', 10)
  const inspectionTimer = due.find(
    (j) => (j.payload as any)?.nodeId === 'inspection_deadline_timer',
  )
  assert.ok(inspectionTimer, 'the inspection deadline timer should be due')
  assert.equal(due.filter((j) => (j.payload as any)?.nodeId === 'inspection_deadline_timer').length, 1)

  await engine.fireTimerJob({ jobId: inspectionTimer.id, workerId: 'worker' })
  assert.ok(hasTask(fake, 'Inspection Deadline Escalation'), 'passed deadline escalates')

  // Amend: extend the inspection deadline — the same instance continues.
  await engine.completeTask({
    taskId: taskByName(fake, 'Inspection Deadline Escalation').id,
    userId: 'broker',
    formData: { inspectionDeadline: '2030-06-01T00:00:00.000Z' },
    transitionName: 'extend',
  })
  assert.ok(
    re.calls.some((c) => c.commandType === 'deal.set_inspection_deadline'),
    'deal.set_inspection_deadline must be issued on amendment',
  )
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.status, 'active', 'the instance must not restart when the deadline changes')

  const pending = pendingTimers(fake, processInstanceId)
  assert.equal(pending.length, 1, 'exactly one re-armed inspection timer')
  assert.equal(
    new Date(pending[0].due_at).toISOString(),
    '2030-06-01T00:00:00.000Z',
    'the re-armed timer uses the amended canonical date',
  )
})

test('M2. no canonical inspection deadline creates NO timer (nothing invented)', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      inspectionApplicable: true,
      inspectionDeadlineScheduled: false,
      financingDeadlineScheduled: false,
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Inspection'), 'inspection track is active')
  const timers = pendingTimers(fake, processInstanceId)
  assert.equal(timers.length, 0, 'no deadline timer without a canonical date')
})

test('M3. financing deadline monitor escalates on passage; amendment re-arms the SAME instance', async () => {
  const { fake, engine, re } = setup(
    simpleCashFacts({
      financingApplicable: true,
      financingDeadlineScheduled: true,
      financingDeadline: '2020-01-01T00:00:00.000Z',
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  assert.ok(hasTask(fake, 'Financing'), 'financing track is active')
  const due = await engine.claimJobs('worker', 10)
  const financingTimer = due.find(
    (j) => (j.payload as any)?.nodeId === 'financing_deadline_timer',
  )
  assert.ok(financingTimer, 'the financing deadline timer should be due')

  await engine.fireTimerJob({ jobId: financingTimer.id, workerId: 'worker' })
  assert.ok(hasTask(fake, 'Financing Deadline Escalation'), 'passed deadline escalates')

  await engine.completeTask({
    taskId: taskByName(fake, 'Financing Deadline Escalation').id,
    userId: 'broker',
    formData: { financingDeadline: '2030-07-01T00:00:00.000Z' },
    transitionName: 'extend',
  })
  assert.ok(
    re.calls.some((c) => c.commandType === 'deal.set_financing_deadline'),
    'deal.set_financing_deadline must be issued on amendment',
  )
  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.status, 'active', 'the instance must not restart when the deadline changes')

  const pending = pendingTimers(fake, processInstanceId)
  assert.equal(pending.length, 1, 'exactly one re-armed financing timer')
  assert.equal(new Date(pending[0].due_at).toISOString(), '2030-07-01T00:00:00.000Z')
})

test('M4. completing the milestone skips the active optional monitor and cancels its pending timer', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      inspectionApplicable: true,
      inspectionDeadlineScheduled: true,
      // Future date => the inspection deadline timer stays pending.
      inspectionDeadline: '2099-01-01T00:00:00.000Z',
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  const monitor = fake.store.jobs.find(
    (j) => j.process_instance_id === processInstanceId
      && j.type === 'timer'
      && (j.payload as any)?.nodeId === 'inspection_deadline_timer',
  )
  assert.ok(monitor, 'the inspection deadline timer was scheduled from the canonical date')
  assert.equal(monitor.status, 'pending')

  // Complete the inspection and every required track; the join must skip the
  // still-active optional deadline monitor and cancel its pending timer.
  await engine.completeTask({
    taskId: taskByName(fake, 'Inspection').id,
    userId: 'inspector',
    transitionName: 'done',
  })
  await completeTasks(engine, fake, REQUIRED_TRACKS)

  const after = fake.store.jobs.find((j) => j.id === monitor.id)
  assert.equal(after!.status, 'cancelled', 'join skip must cancel the pending deadline timer')
  assert.ok(!hasOpenTask(fake, 'Inspection Deadline Escalation'))
})

test('M5. cash/no-financing deals never start a financing deadline timer', async () => {
  const { fake, engine } = setup(
    simpleCashFacts({
      financingApplicable: false,
      financingDeadlineScheduled: true, // canonical date recorded, but cash deal
      financingDeadline: '2026-09-01T00:00:00.000Z',
    }),
  )
  const processInstanceId = await startAndSignContract(engine, fake)

  const timers = pendingTimers(fake, processInstanceId)
  assert.equal(
    timers.filter((j) => (j.payload as any)?.nodeId === 'financing_deadline_timer').length,
    0,
    'a cash deal must never monitor a financing deadline',
  )
})
