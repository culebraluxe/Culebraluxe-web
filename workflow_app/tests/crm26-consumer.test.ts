import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  Crm26AgreementExecutionConsumer,
  Crm26RejectError,
  completePnsExecutedTask,
  type Crm26ConsumerDeps,
  type IssuedAgreementDocument,
} from '../../lib/agreements/crm26-consumer'
import {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_DEADLINE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_INSPECTION_DEADLINE,
  DEAL_SET_STAGE_UNDER_CONTRACT,
} from '../../lib/commands/command-types'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator, makeApp } from '../../workflow_engine/tests/fixtures'
import {
  parseReSupermodel,
  RE_SUPERMODEL_KEY,
  RE_SUPERMODEL_VERSION,
} from '../definitions/re-supermodel'
import type { MqDeliveryContext, MqMessage } from '../../lib/mq/types'
import type { CommandEnvelope, CommandResult } from '../../lib/commands/contracts'

// ---------------------------------------------------------------------------
// CRM-26 — agreement-execution consumer proofs (tests 4-12).
// Tests 4-10 use injected fakes for every seam. Tests 11-12 drive the REAL
// WorkflowEngine over the RE_supermodel XML through the in-memory FakeSql.
// ---------------------------------------------------------------------------

const CONTEXT: MqDeliveryContext = {
  deliveryId: 'del-1',
  subscriptionId: 'crm26-agreement-execution',
  attempt: 1,
  maxAttempts: 3,
}

function message(overrides: Partial<MqMessage> = {}): MqMessage {
  return {
    messageId: 'evt-1',
    routingKey: 'AGREEMENT_FULLY_EXECUTED',
    payload: {
      transactionDocumentId: 'doc-1',
      issuedVersion: 1,
      templateId: 'PR-PNS',
      dealId: 'deal-1',
    },
    occurredAt: '2026-08-24T12:00:00.000Z',
    correlationId: 'corr-1',
    causationId: 'cmd-1',
    aggregateType: 'transaction_document',
    aggregateId: 'doc-1',
    ...overrides,
  }
}

function doc(overrides: Partial<IssuedAgreementDocument> = {}): IssuedAgreementDocument {
  return {
    id: 'doc-1',
    dealId: 'deal-1',
    templateId: 'PR-PNS',
    issuedVersion: 1,
    sourceSnapshot: {
      fieldValues: {
        closingDate: '2026-08-31',
        inspectionDeadline: '2026-08-10',
        financingDeadline: '2026-08-15',
        financing: 'Financed',
        appraisalWaived: 'No',
        purchasePrice: '450000',
        surveyDeadline: '2026-08-20',
      },
    },
    ...overrides,
  }
}

/** A fake canonical Deal dispatcher: applies the 5 operational commands, replays on duplicate commandId. */
class FakeDealDispatcher {
  calls: CommandEnvelope[] = []
  applied = new Map<string, number>() // commandId -> mutation count (idempotency)
  failOn = new Set<string>()
  deal: Record<string, unknown> = { id: 'deal-1', stage: 'offer' }

  async execute(envelope: CommandEnvelope): Promise<CommandResult> {
    this.calls.push(envelope)
    if (this.failOn.has(envelope.commandId)) {
      throw new Error(`fake dispatch failure: ${envelope.commandId}`)
    }
    if (this.applied.has(envelope.commandId)) {
      return {
        commandId: envelope.commandId,
        outcome: 'success',
        emittedEvents: [],
        aggregateId: envelope.aggregateId,
        message: null,
        replayed: true,
      }
    }
    this.applied.set(envelope.commandId, 1)
    const input = envelope.input as Record<string, any>
    switch (envelope.commandType) {
      case DEAL_SET_CLOSING_DATE:
        this.deal.closingDate = input.closingDate
        break
      case DEAL_SET_INSPECTION_DEADLINE:
        this.deal.inspectionDeadline = input.inspectionDeadline
        break
      case DEAL_SET_FINANCING_DEADLINE:
        this.deal.financingDeadline = input.financingDeadline
        break
      case DEAL_SET_FINANCING_TYPE:
        this.deal.financingType = input.financingType
        break
      case DEAL_SET_APPRAISAL_REQUIRED:
        this.deal.appraisalRequired = input.appraisalRequired
        break
      default:
        throw new Error(`consumer must never dispatch ${envelope.commandType}`)
    }
    return {
      commandId: envelope.commandId,
      outcome: 'success',
      emittedEvents: [],
      aggregateId: envelope.aggregateId,
      message: null,
      replayed: false,
    }
  }
}

function makeConsumer(
  overrides: Partial<Crm26ConsumerDeps> = {},
  fake = new FakeDealDispatcher(),
): { consumer: Crm26AgreementExecutionConsumer; fake: FakeDealDispatcher } {
  const deps: Crm26ConsumerDeps = {
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeCommand: (e) => fake.execute(e),
    completePnsExecuted: async () => {},
    ...overrides,
  }
  return { consumer: new Crm26AgreementExecutionConsumer(deps), fake }
}



async function rejectsReason(
  overrides: Partial<Crm26ConsumerDeps>,
  expectedReason: Crm26RejectError['reason'],
): Promise<void> {
  const deps: Crm26ConsumerDeps = {
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeCommand: async () => ({ outcome: 'success' as const } as CommandResult),
    completePnsExecuted: async () => {},
    ...overrides,
  }
  const consumer = new Crm26AgreementExecutionConsumer(deps)
  await assert.rejects(
    () => consumer.handle(message(), CONTEXT),
    (err: unknown) => err instanceof Crm26RejectError && err.reason === expectedReason,
    `expected Crm26RejectError with reason '${expectedReason}'`,
  )
}

test('CRM-26 test 4: wrong template / document / version / deal lineage is rejected truthfully', async () => {
  await rejectsReason(
    {
      loadIssuedDocument: async () => null,
      executionMarkerMatches: async () => true,
      executeCommand: async () => ({ outcome: 'success' as const } as CommandResult),
      completePnsExecuted: async () => {},
    },
    'document_not_found',
  )

  await rejectsReason(
    { loadIssuedDocument: async () => doc({ templateId: 'OFFER-01' }) },
    'template_mismatch',
  )
  await rejectsReason(
    { loadIssuedDocument: async () => doc({ issuedVersion: 2 }) },
    'version_mismatch',
  )
  await rejectsReason(
    { loadIssuedDocument: async () => doc({ dealId: 'deal-9' }) },
    'deal_mismatch',
  )
  await rejectsReason(
    { executionMarkerMatches: async () => false },
    'execution_marker_mismatch',
  )
})

test('CRM-26 test 4b: a matching but non-eligible template (non-PR-PNS) is rejected', async () => {
  // Payload + document both name OFFER-01 (so lineage matches) but OFFER-01 is not
  // an execution-eligible agreement -> template_not_eligible, never a projection.
  const offerMessage = message({
    payload: {
      transactionDocumentId: 'doc-1',
      issuedVersion: 1,
      templateId: 'OFFER-01',
      dealId: 'deal-1',
    },
  })
  const consumer = new Crm26AgreementExecutionConsumer({
    loadIssuedDocument: async () => doc({ templateId: 'OFFER-01' }),
    executionMarkerMatches: async () => true,
    executeCommand: async () => {
      throw new Error('must not dispatch for an ineligible template')
    },
    completePnsExecuted: async () => {
      throw new Error('must not advance for an ineligible template')
    },
  })
  await assert.rejects(
    () => consumer.handle(offerMessage, CONTEXT),
    (err: unknown) => err instanceof Crm26RejectError && err.reason === 'template_not_eligible',
  )
})




test('CRM-26 test 5: each supported field invokes its existing canonical command', async () => {
  const { consumer, fake } = makeConsumer()
  await consumer.handle(message(), CONTEXT)

  const types = fake.calls.map((c) => c.commandType)
  assert.deepEqual(
    new Set(types),
    new Set([
      DEAL_SET_CLOSING_DATE,
      DEAL_SET_INSPECTION_DEADLINE,
      DEAL_SET_FINANCING_DEADLINE,
      DEAL_SET_FINANCING_TYPE,
      DEAL_SET_APPRAISAL_REQUIRED,
    ]),
  )
  assert.ok(!types.includes(DEAL_SET_STAGE_UNDER_CONTRACT), 'consumer must not set stage')

  const byType = (t: string) => fake.calls.find((c) => c.commandType === t)!
  assert.deepEqual(byType(DEAL_SET_CLOSING_DATE).input, { closingDate: '2026-08-31' })
  assert.deepEqual(byType(DEAL_SET_INSPECTION_DEADLINE).input, { inspectionDeadline: '2026-08-10' })
  assert.deepEqual(byType(DEAL_SET_FINANCING_DEADLINE).input, { financingDeadline: '2026-08-15' })
  assert.deepEqual(byType(DEAL_SET_FINANCING_TYPE).input, { financingType: 'financed' })
  assert.deepEqual(byType(DEAL_SET_APPRAISAL_REQUIRED).input, { appraisalRequired: true })

  // Correlation/causation preserved from the message.
  for (const c of fake.calls) {
    assert.equal(c.aggregateType, 'deal')
    assert.equal(c.aggregateId, 'deal-1')
    assert.equal(c.correlationId, 'corr-1')
    assert.equal(c.causationId, 'evt-1')
    assert.equal(c.actorAppUserId, null, 'system actor is null (no app_user)')
  }
})

test('CRM-26 test 6: deterministic command ids make an identical event replay harmless', async () => {
  const { consumer, fake } = makeConsumer()

  await consumer.handle(message(), CONTEXT)
  await consumer.handle(message(), CONTEXT) // identical re-delivery

  const first = fake.calls.slice(0, 5).map((c) => c.commandId)
  const second = fake.calls.slice(5, 10).map((c) => c.commandId)

  assert.deepEqual(second, first, 'a replay must reuse the exact same deterministic command ids')
  assert.equal(fake.applied.size, 5, 'only 5 distinct commands applied across both deliveries')
  for (const [id, count] of fake.applied) {
    assert.equal(count, 1, `command ${id} must mutate exactly once`)
  }
  assert.equal(fake.deal.closingDate, '2026-08-31')
  assert.equal(fake.deal.financingType, 'financed')
})

test('CRM-26 test 7: a partial projection failure retries without duplicate mutation', async () => {
  const { consumer, fake } = makeConsumer()
  fake.failOn.add('evt-1:financing')

  // First delivery: closes/inspection/financingDeadline apply, financing throws.
  await assert.rejects(() => consumer.handle(message(), CONTEXT), /fake dispatch failure/)
  assert.equal(fake.deal.closingDate, '2026-08-31', 'earlier facts applied before the failure')
  assert.equal(fake.deal.financingType, undefined, 'financing not applied on the failed delivery')
  assert.equal(fake.applied.size, 3, '3 commands applied before the failure')

  // Retry after the transient cause clears: prior commands replay (no re-mutation).
  fake.failOn.clear()
  await consumer.handle(message(), CONTEXT)
  assert.equal(fake.applied.size, 5, 'exactly 5 distinct commands in the end')
  assert.equal(fake.applied.get('evt-1:closingDate'), 1, 'closingDate mutated exactly once')
  assert.equal(fake.deal.financingType, 'financed')
  assert.equal(fake.deal.appraisalRequired, true)
})


// ---------------------------------------------------------------------------
// completePnsExecutedTask — workflow correlation / reuse / replay safety.
// ---------------------------------------------------------------------------

test('CRM-26 test 8: the existing correlated workflow instance is reused (never recreated)', async () => {
  const completed: Array<[string, string, string]> = []
  await completePnsExecutedTask('deal-1', {
    findActiveInstance: async (dealId) => {
      assert.equal(dealId, 'deal-1')
      return 'inst-1'
    },
    findActionableTask: async (instanceId, nodeId) => {
      assert.equal(instanceId, 'inst-1')
      assert.equal(nodeId, 'pns_executed')
      return 'task-pns'
    },
    completeEngineTask: async (taskId, userId, transition) => {
      completed.push([taskId, userId, transition])
    },
  })
  assert.deepEqual(completed, [['task-pns', 'system', 'executed']])
})

test('CRM-26 test 9: pns_executed completes exactly once across an identical replay', async () => {
  let actionable = true
  const completed: string[] = []
  const deps = {
    findActiveInstance: async () => 'inst-1' as string | null,
    findActionableTask: async () => (actionable ? 'task-pns' : null),
    completeEngineTask: async (taskId: string) => {
      completed.push(taskId)
      actionable = false
    },
  }
  await completePnsExecutedTask('deal-1', deps)
  await completePnsExecutedTask('deal-1', deps)
  assert.equal(completed.length, 1, 'the pns_executed task must be completed exactly once')
})

test('CRM-26 test 9b: an already-completed task (TASK_ALREADY_COMPLETED) is recovery success', async () => {
  await completePnsExecutedTask('deal-1', {
    findActiveInstance: async () => 'inst-1',
    findActionableTask: async () => 'task-pns',
    completeEngineTask: async () => {
      throw Object.assign(new Error('already completed'), { code: 'TASK_ALREADY_COMPLETED' })
    },
  })
  // Must not throw: duplicate completion is treated as successful recovery.
})

test('CRM-26 test 10: no duplicate workflow instance is created', async () => {
  // The helper only ever reuses an existing active instance and never starts one.
  let instanceChecks = 0
  await completePnsExecutedTask('deal-1', {
    findActiveInstance: async () => {
      instanceChecks += 1
      return 'inst-1'
    },
    findActionableTask: async () => null, // already advanced past pns_executed -> recovery
    completeEngineTask: async () => {
      throw new Error('must not complete when already advanced')
    },
  })
  assert.equal(instanceChecks, 1, 'one instance lookup, no new instance creation')
})


// ---------------------------------------------------------------------------
// E2E — real WorkflowEngine over the RE_supermodel XML + in-memory FakeSql.
// ---------------------------------------------------------------------------

function applyOperationalCommandToDeal(
  deal: Record<string, unknown>,
  envelope: CommandEnvelope,
): CommandResult {
  const input = envelope.input as Record<string, any>
  switch (envelope.commandType) {
    case DEAL_SET_CLOSING_DATE:
      deal.closingDate = input.closingDate
      break
    case DEAL_SET_INSPECTION_DEADLINE:
      deal.inspectionDeadline = input.inspectionDeadline
      break
    case DEAL_SET_FINANCING_DEADLINE:
      deal.financingDeadline = input.financingDeadline
      break
    case DEAL_SET_FINANCING_TYPE:
      deal.financingType = input.financingType
      break
    case DEAL_SET_APPRAISAL_REQUIRED:
      deal.appraisalRequired = input.appraisalRequired
      break
    default:
      // The CRM-26 consumer must never set the stage itself.
      throw new Error(`consumer must not dispatch ${envelope.commandType}`)
  }
  return {
    commandId: envelope.commandId,
    outcome: 'success' as const,
    emittedEvents: [],
    aggregateId: envelope.aggregateId,
    message: null,
    replayed: false,
  }
}

function pnsExecutedTaskId(fake: FakeSql, instanceId: string): string | null {
  const t = fake.store.tasks.find(
    (row) =>
      row.process_instance_id === instanceId &&
      row.name === 'Contract / P&S Executed' &&
      ['ready', 'reserved', 'in_progress'].includes(row.status),
  )
  return t ? (t.id as string) : null
}

/** Start the RE_supermodel for deal-1 and complete pns_preparation, leaving pns_executed actionable. */
async function startToPnsExecuted(
  deal: Record<string, unknown>,
  fake: FakeSql,
): Promise<{ engine: WorkflowEngine; engineCalls: any[] }> {
  const parsed = parseReSupermodel()
  fake.seedDefinition(RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION, parsed.graph)
  const engineCalls: any[] = []
  const app = makeApp({
    executeCommand: async (req: any) => {
      engineCalls.push(req)
      if (req.commandType === DEAL_SET_STAGE_UNDER_CONTRACT) deal.stage = 'under_contract'
      return { commandId: req.commandId, outcome: 'success' as const }
    },
  })
  const engine = new WorkflowEngine(fake.sql as any, { evaluate: stubEvaluator, app: app as any })

  await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'system',
    variables: {},
    subject: { subjectType: 'deal', subjectId: 'deal-1' },
  })
  const prepTask = fake.store.tasks.find((t) => t.name === 'Contract / P&S Preparation')
  assert.ok(prepTask, 'pns_preparation task exists after start')
  await engine.completeTask({
    taskId: prepTask.id as string,
    userId: 'system',
    transitionName: 'prepared',
  })
  return { engine, engineCalls }
}


test('CRM-26 test 12 (E2E): accepted offer -> existing workflow -> executed PR-PNS -> Deal facts -> pns_executed -> under_contract', async () => {
  const deal: Record<string, unknown> = { id: 'deal-1', stage: 'offer' }
  const fake = new FakeSql()
  const { engine } = await startToPnsExecuted(deal, fake)

  // Exactly one active instance must already exist (accepted-offer workflow).
  const instancesForDeal = fake.store.processInstances.filter(
    (r) => r.subject_id === 'deal-1' && r.status === 'active',
  )
  assert.equal(instancesForDeal.length, 1, 'one existing workflow instance')

  const consumer = new Crm26AgreementExecutionConsumer({
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeCommand: async (e) => applyOperationalCommandToDeal(deal, e),
    completePnsExecuted: (dealId) =>
      completePnsExecutedTask(dealId, {
        findActiveInstance: async (id) => {
          const inst = fake.store.processInstances.find(
            (r) => r.subject_id === id && r.status === 'active',
          )
          return inst ? (inst.id as string) : null
        },
        findActionableTask: async (instanceId) => pnsExecutedTaskId(fake, instanceId),
        completeEngineTask: async (taskId, userId, transition) => {
          await engine.completeTask({ taskId, userId, transitionName: transition })
        },
      }),
  })

  await consumer.handle(message(), CONTEXT)

  // Canonical Deal facts projected from the immutable PR-PNS snapshot.
  assert.equal(deal.closingDate, '2026-08-31')
  assert.equal(deal.inspectionDeadline, '2026-08-10')
  assert.equal(deal.financingDeadline, '2026-08-15')
  assert.equal(deal.financingType, 'financed')
  assert.equal(deal.appraisalRequired, true)

  // pns_executed task completed exactly once.
  const pnsTask = fake.store.tasks.find(
    (t) => t.name === 'Contract / P&S Executed' && t.status === 'completed',
  )
  assert.ok(pnsTask, 'pns_executed completed')

  // Deal reached under_contract via mark_under_contract (not the consumer).
  assert.equal(deal.stage, 'under_contract')

  // Replaying the same AGREEMENT_FULLY_EXECUTED message must not create a duplicate
  // instance or re-advance anything (the task is no longer actionable -> recovery).
  await consumer.handle(message(), CONTEXT)
  const instancesAfter = fake.store.processInstances.filter(
    (r) => r.subject_id === 'deal-1' && r.status === 'active',
  )
  assert.equal(instancesAfter.length, 1, 'replay creates no duplicate instance')
  assert.equal(deal.stage, 'under_contract', 'replay does not re-run mark_under_contract')
})

test('CRM-26 test 11: the existing mark_under_contract node (not the consumer) changes Deal stage', async () => {
  const deal: Record<string, unknown> = { id: 'deal-1', stage: 'offer' }
  const fake = new FakeSql()
  const { engine, engineCalls } = await startToPnsExecuted(deal, fake)

  const consumer = new Crm26AgreementExecutionConsumer({
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeCommand: async (e) => applyOperationalCommandToDeal(deal, e),
    completePnsExecuted: (dealId) =>
      completePnsExecutedTask(dealId, {
        findActiveInstance: async (id) => {
          const inst = fake.store.processInstances.find(
            (r) => r.subject_id === id && r.status === 'active',
          )
          return inst ? (inst.id as string) : null
        },
        findActionableTask: async (instanceId) => pnsExecutedTaskId(fake, instanceId),
        completeEngineTask: async (taskId, userId, transition) => {
          await engine.completeTask({ taskId, userId, transitionName: transition })
        },
      }),
  })

  await consumer.handle(message(), CONTEXT)

  // The consumer's own executeCommand only ever received the 5 operational commands
  // (applyOperationalCommandToDeal throws on anything else), so the stage change must
  // have come from the engine's mark_under_contract command-node.
  assert.equal(deal.stage, 'under_contract', 'mark_under_contract moved the Deal to under_contract')
  assert.ok(
    engineCalls.some((c) => c.commandType === DEAL_SET_STAGE_UNDER_CONTRACT),
    'the engine dispatched deal.set_stage_under_contract via its app port',
  )
})




