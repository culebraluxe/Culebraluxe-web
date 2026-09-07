import { test } from 'node:test'
import assert from 'node:assert/strict'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../testv2/engine_tests/fake-sql'
import { stubEvaluator } from '../../testv2/engine_tests/fixtures'
import {
  parseReSupermodel,
  RE_SUPERMODEL_KEY,
  RE_SUPERMODEL_VERSION,
} from '../definitions/re-supermodel'
import {
  resolveSmeParticipant,
  orchestrateSmeTaskCore,
  type SmeParticipantCandidate,
} from '../sme-orchestration'

// ---------------------------------------------------------------------------
// CRM-15 — External SME orchestration seam.
//
// Two tiers:
//   1. Pure unit coverage of resolveSmeParticipant / orchestrateSmeTaskCore —
//      responsibility hint -> actual deal participant, and materialization of
//      the canonical task ADDRESSED TO the SME (person_id), idempotent,
//      never blocked by an unrecorded participant. No database.
//   2. Engine integration — the RE_supermodel (from XML, not a hand-written
//      graph) through the real WorkflowEngine on the in-memory FakeSql:
//      the appraisal SME task is orchestrated (resolved + materialized with
//      the appraiser attached) and the SME completion advances the closing
//      orchestration through join -> closing readiness -> closing -> closed.
// ---------------------------------------------------------------------------

const participants: SmeParticipantCandidate[] = [
  { id: 'dp-client', role: 'client', roleLabel: null, personId: 'person-buyer', userId: null, active: true },
  { id: 'dp-seller', role: 'seller', roleLabel: null, personId: 'person-seller', userId: null, active: true },
  { id: 'dp-appraiser', role: 'other', roleLabel: 'appraiser', personId: 'person-appraiser', userId: null, active: true },
  { id: 'dp-lender', role: 'other', roleLabel: 'lender', personId: 'person-lender', userId: null, active: true },
  { id: 'dp-title', role: 'other', roleLabel: 'title', personId: 'person-title', userId: null, active: true },
]

// ---------------------------------------------------------------------------
// 1. Pure resolution: hint -> actual SME participant
// ---------------------------------------------------------------------------

test('appraiser hint resolves to the active role=other + role_label=appraiser participant', () => {
  const res = resolveSmeParticipant(participants, 'appraiser')
  assert.equal(res.kind, 'sme')
  if (res.kind !== 'sme') return
  assert.equal(res.participant.id, 'dp-appraiser')
  assert.equal(res.participant.personId, 'person-appraiser')
  assert.equal(res.spec.label, 'Appraiser (external SME)')
})

test('long-tail role_label matching is case-insensitive', () => {
  const res = resolveSmeParticipant(participants, 'appraiser')
  assert.equal(res.kind, 'sme')
  const upper = resolveSmeParticipant(
    participants.map((p) => ({
      ...p,
      roleLabel: p.roleLabel ? p.roleLabel.toUpperCase() : p.roleLabel,
    })),
    'appraiser',
  )
  assert.equal(upper.kind, 'sme')
})

test('lender and title hints resolve to their own role_label participant', () => {
  const lender = resolveSmeParticipant(participants, 'lender')
  assert.equal(lender.kind, 'sme')
  if (lender.kind === 'sme') assert.equal(lender.participant.id, 'dp-lender')

  const title = resolveSmeParticipant(participants, 'title_company')
  assert.equal(title.kind, 'sme')
  if (title.kind === 'sme') assert.equal(title.participant.id, 'dp-title')
})

test('buyer / seller hints resolve to the structural client / seller participant', () => {
  const buyer = resolveSmeParticipant(participants, 'buyer')
  assert.equal(buyer.kind, 'sme')
  if (buyer.kind === 'sme') {
    assert.equal(buyer.participant.id, 'dp-client')
    assert.equal(buyer.target.kind, 'structural')
  }

  const seller = resolveSmeParticipant(participants, 'seller')
  assert.equal(seller.kind, 'sme')
  if (seller.kind === 'sme') {
    assert.equal(seller.participant.id, 'dp-seller')
    assert.equal(seller.target.kind, 'structural')
  }
})

test('brokerage / other_sme / unknown hints have no participant target', () => {
  assert.deepEqual(resolveSmeParticipant(participants, 'brokerage'), {
    kind: 'none',
    reason: 'no_target',
  })
  assert.deepEqual(resolveSmeParticipant(participants, 'other_sme'), {
    kind: 'none',
    reason: 'no_target',
  })
  assert.deepEqual(resolveSmeParticipant(participants, 'mystery_role'), {
    kind: 'none',
    reason: 'no_target',
  })
  assert.deepEqual(resolveSmeParticipant(participants, undefined), {
    kind: 'none',
    reason: 'no_hint',
  })
})

test('a missing or inactive SME participant is a typed no_participant — never invented', () => {
  const withoutAppraiser = participants.filter((p) => p.id !== 'dp-appraiser')
  assert.deepEqual(resolveSmeParticipant(withoutAppraiser, 'appraiser'), {
    kind: 'none',
    reason: 'no_participant',
  })

  const inactiveAppraiser = participants.map((p) =>
    p.id === 'dp-appraiser' ? { ...p, active: false } : p,
  )
  assert.deepEqual(resolveSmeParticipant(inactiveAppraiser, 'appraiser'), {
    kind: 'none',
    reason: 'no_participant',
  })
})

// ---------------------------------------------------------------------------
// 2. Pure orchestration core: materialize the task addressed to the SME
// ---------------------------------------------------------------------------

function makeMaterialize() {
  const created: Array<Record<string, unknown>> = []
  const correlated = new Set<string>()
  return {
    created,
    // Mirrors materializeEngineTask: a task that is already correlated
    // returns the existing application task WITHOUT creating a canonical task.
    materialize: async (input: any) => {
      if (correlated.has(input.workflowTaskId)) {
        return { applicationTaskId: `existing-${input.workflowTaskId}`, created: false }
      }
      correlated.add(input.workflowTaskId)
      created.push(input)
      return { applicationTaskId: `task-${created.length}`, created: true }
    },
  }
}

test('orchestrateSmeTaskCore materializes the canonical task addressed to the resolved SME', async () => {
  const { created, materialize } = makeMaterialize()

  const result = await orchestrateSmeTaskCore(
    {
      workflowTaskId: 'wt-appraisal',
      title: 'Appraisal',
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    { participants, materialize },
  )

  assert.equal(result.created, true)
  assert.equal(result.applicationTaskId, 'task-1')
  assert.equal(result.sme.kind, 'sme')
  // The canonical task carries the SME person (task.person_id = appraiser).
  assert.equal(created[0].personId, 'person-appraiser')
  assert.equal(created[0].dealId, 'deal-1')
})

test('orchestrateSmeTaskCore retry returns the existing correlation without duplicating', async () => {
  const { created, materialize } = makeMaterialize()

  const first = await orchestrateSmeTaskCore(
    {
      workflowTaskId: 'wt-appraisal',
      title: 'Appraisal',
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    { participants, materialize },
  )
  assert.equal(first.created, true)

  const retry = await orchestrateSmeTaskCore(
    {
      workflowTaskId: 'wt-appraisal',
      title: 'Appraisal',
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    { participants, materialize },
  )
  assert.equal(retry.created, false)
  assert.equal(created.length, 1, 'a retry must not create a second canonical task')
  assert.equal(retry.sme.kind, 'sme')
})

test('an unrecorded SME never blocks materialization — the task still materializes SME-less', async () => {
  const { created, materialize } = makeMaterialize()
  const withoutAppraiser = participants.filter((p) => p.id !== 'dp-appraiser')

  const result = await orchestrateSmeTaskCore(
    {
      workflowTaskId: 'wt-appraisal',
      title: 'Appraisal',
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    { participants: withoutAppraiser, materialize },
  )

  assert.equal(result.created, true)
  assert.deepEqual(result.sme, { kind: 'none', reason: 'no_participant' })
  assert.equal(created[0].personId, undefined, 'no SME to address the task to')
})

// ---------------------------------------------------------------------------
// 3. Engine integration: SME orchestration advances the closing lifecycle
// ---------------------------------------------------------------------------

type AnyRow = Record<string, any>

function makeReApp(initial: Record<string, any>) {
  let facts: Record<string, any> = { ...initial }
  const calls: any[] = []
  return {
    calls,
    app: {
      async executeCommand(req: any) {
        calls.push(req)
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
  return { fake, engine, graph: parsed.graph }
}

function taskByName(fake: FakeSql, name: string): AnyRow {
  const list = fake.store.tasks.filter((t) => t.name === name)
  const t = list[list.length - 1]
  assert.ok(t, `expected task "${name}" to exist`)
  return t!
}

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

async function completeRequiredTracks(engine: WorkflowEngine, fake: FakeSql) {
  for (const name of ['Title / Legal', 'Tax / Municipal Clearance', 'Funds Ready', 'Closing Documents']) {
    await engine.completeTask({
      taskId: taskByName(fake, name).id,
      userId: 'sme',
      transitionName: 'done',
    })
  }
}

test('CRM-15: appraisal SME orchestration — resolve, materialize with the SME, complete, and advance to closing', async () => {
  const { fake, engine } = setup({
    // Cash + appraisal: appraisal branch active, everything else skipped.
    financingApplicable: false,
    appraisalApplicable: true,
    inspectionApplicable: false,
    insuranceApplicable: false,
    requiresSurvey: false,
    requiresHoaClearance: false,
    requiresRegistryFollowup: false,
    closingConfirmationRequired: false,
    closingDateScheduled: false,
    closingDocumentsReady: true,
  })
  const processInstanceId = await startAndSignContract(engine, fake)

  // The fork spawned the appraisal SME task (and the four required tracks).
  assert.ok(hasOpenTask(fake, 'Appraisal'))
  assert.ok(hasOpenTask(fake, 'Title / Legal'))
  assert.equal(hasOpenTask(fake, 'Financing'), false)

  // --- SME orchestration: resolve the responsible SME and materialize the
  // canonical task ADDRESSED TO the appraiser (this is what the
  // reconciliation path now does for every active engine task). ---
  const appraisalTask = taskByName(fake, 'Appraisal')
  const materialized: Array<Record<string, unknown>> = []
  const result = await orchestrateSmeTaskCore(
    {
      workflowTaskId: appraisalTask.id,
      title: appraisalTask.name,
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    {
      participants,
      materialize: async (input) => {
        materialized.push(input)
        return { applicationTaskId: `canonical-${input.workflowTaskId}`, created: true }
      },
    },
  )

  assert.equal(result.created, true)
  assert.equal(result.sme.kind, 'sme')
  assert.equal(materialized[0].personId, 'person-appraiser', 'the canonical task is addressed to the appraiser')

  // --- SME completion: complete the appraisal through the engine completion
  // path (the exact call completeWorkflowTask performs). ---
  await engine.completeTask({
    taskId: appraisalTask.id,
    userId: 'person-appraiser',
    transitionName: 'done',
  })
  assert.equal(hasOpenTask(fake, 'Appraisal'), false)

  // --- The closing orchestration continues: remaining required tracks
  // complete, the join releases, gates clear, and closing proceeds. ---
  await completeRequiredTracks(engine, fake)
  assert.ok(hasOpenTask(fake, 'Closing'), 'after the SME completes, the workflow reaches Closing')

  await engine.completeTask({
    taskId: taskByName(fake, 'Closing').id,
    userId: 'broker',
    transitionName: 'closed',
  })

  const pi = await engine.getProcessInstance(processInstanceId)
  assert.equal(pi!.outcome, 'completed')
})

test('CRM-15: an SME task with no recorded participant still materializes and completes (never blocks)', async () => {
  const { fake, engine } = setup({
    financingApplicable: false,
    appraisalApplicable: true,
    inspectionApplicable: false,
    insuranceApplicable: false,
    requiresSurvey: false,
    requiresHoaClearance: false,
    requiresRegistryFollowup: false,
    closingConfirmationRequired: false,
    closingDateScheduled: false,
    closingDocumentsReady: true,
  })
  await startAndSignContract(engine, fake)

  const appraisalTask = taskByName(fake, 'Appraisal')
  // No appraiser participant on the deal.
  const result = await orchestrateSmeTaskCore(
    {
      workflowTaskId: appraisalTask.id,
      title: appraisalTask.name,
      subjectType: 'deal',
      subjectId: 'deal-1',
      dealId: 'deal-1',
      responsibilityHint: 'appraiser',
    },
    {
      participants: participants.filter((p) => p.id !== 'dp-appraiser'),
      materialize: async () => ({ applicationTaskId: 'canonical-x', created: true }),
    },
  )
  assert.deepEqual(result.sme, { kind: 'none', reason: 'no_participant' })

  // The workflow itself is untouched by the missing participant: the SME task
  // can still be completed and the transaction can still close.
  await engine.completeTask({
    taskId: appraisalTask.id,
    userId: 'broker',
    transitionName: 'done',
  })
  await completeRequiredTracks(engine, fake)
  assert.ok(hasOpenTask(fake, 'Closing'))
})
