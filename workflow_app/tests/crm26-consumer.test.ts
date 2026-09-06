import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  Crm26AgreementExecutionConsumer,
  Crm26RejectError,
  completePnsExecutedTask,
  type Crm26ConsumerDeps,
  type IssuedAgreementDocument,
} from '../../lib/agreements/crm26-consumer'
import { DEAL_SET_STAGE_UNDER_CONTRACT } from '../../lib/commands/command-types'
import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../workflow_engine/tests/fake-sql'
import { stubEvaluator, makeApp } from '../../workflow_engine/tests/fixtures'
import {
  parseReSupermodel,
  RE_SUPERMODEL_KEY,
  RE_SUPERMODEL_VERSION,
} from '../definitions/re-supermodel'
import { createApplicationPort } from '../application-port'
import type { MqDeliveryContext, MqMessage } from '../../lib/mq/types'

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
      contractId: 'contract-1',
    },
    occurredAt: '2026-09-06T12:00:00.000Z',
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
    contractId: 'contract-1',
    templateId: 'PR-PNS',
    issuedVersion: 1,
    ...overrides,
  }
}

function makeConsumer(overrides: Partial<Crm26ConsumerDeps> = {}) {
  const calls: string[] = []
  const deps: Crm26ConsumerDeps = {
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeContract: async (contractId, evidenceDocumentId) => {
      calls.push(`execute:${contractId}:${evidenceDocumentId}`)
    },
    ensureWorkflow: async (contractId) => {
      calls.push(`workflow:${contractId}`)
    },
    completePnsExecuted: async (contractId) => {
      calls.push(`complete:${contractId}`)
    },
    ...overrides,
  }
  return { consumer: new Crm26AgreementExecutionConsumer(deps), calls }
}

async function rejectsReason(
  overrides: Partial<Crm26ConsumerDeps>,
  expectedReason: Crm26RejectError['reason'],
  event = message(),
): Promise<void> {
  const { consumer } = makeConsumer(overrides)
  await assert.rejects(
    () => consumer.handle(event, CONTEXT),
    (err: unknown) => err instanceof Crm26RejectError && err.reason === expectedReason,
  )
}

test('CRM26 Contract cut: payload requires explicit Contract lineage', async () => {
  await assert.rejects(
    () => makeConsumer().consumer.handle(
      message({
        payload: {
          transactionDocumentId: 'doc-1',
          issuedVersion: 1,
          templateId: 'PR-PNS',
          dealId: 'deal-1',
        },
      }),
      CONTEXT,
    ),
    (err: unknown) => err instanceof Crm26RejectError && err.reason === 'malformed_payload',
  )
})

test('CRM26 Contract cut: immutable document lineage is checked against contractId', async () => {
  await rejectsReason({ loadIssuedDocument: async () => null }, 'document_not_found')
  await rejectsReason(
    { loadIssuedDocument: async () => doc({ templateId: 'OFFER-01' }) },
    'template_mismatch',
  )
  await rejectsReason(
    { loadIssuedDocument: async () => doc({ issuedVersion: 2 }) },
    'version_mismatch',
  )
  await rejectsReason(
    { loadIssuedDocument: async () => doc({ contractId: 'contract-9' }) },
    'contract_mismatch',
  )
  await rejectsReason({ executionMarkerMatches: async () => false }, 'execution_marker_mismatch')
})

test('CRM26 Contract cut: execution order is Contract.execute -> Contract workflow -> pns_executed', async () => {
  const { consumer, calls } = makeConsumer()
  await consumer.handle(message(), CONTEXT)
  assert.deepEqual(calls, [
    'execute:contract-1:doc-1',
    'workflow:contract-1',
    'complete:contract-1',
  ])
})

test('CRM26 Contract cut: Contract.execute failure stops workflow advancement', async () => {
  const calls: string[] = []
  await rejectsReason(
    {
      executeContract: async () => {
        calls.push('execute')
        throw new Error('write failed')
      },
      ensureWorkflow: async () => calls.push('workflow'),
      completePnsExecuted: async () => calls.push('complete'),
    },
    'contract_execution_failed',
  )
  assert.deepEqual(calls, ['execute'])
})

test('CRM26 Contract cut: P&S preparation is caught up before execution task', async () => {
  const active = new Map<string, string>([
    ['pns_preparation', 'task-prep'],
    ['pns_executed', 'task-exec'],
  ])
  const completed: Array<[string, string]> = []
  await completePnsExecutedTask('contract-1', {
    findActiveInstance: async (contractId) => {
      assert.equal(contractId, 'contract-1')
      return 'inst-1'
    },
    findActionableTask: async (_instanceId, nodeId) => active.get(nodeId) ?? null,
    completeEngineTask: async (taskId, _userId, transition) => {
      completed.push([taskId, transition])
      if (taskId === 'task-prep') active.delete('pns_preparation')
      if (taskId === 'task-exec') active.delete('pns_executed')
    },
  })
  assert.deepEqual(completed, [
    ['task-prep', 'prepared'],
    ['task-exec', 'executed'],
  ])
})

test('CRM26 Contract cut: task completion is replay-safe', async () => {
  let actionable = true
  let completed = 0
  const deps = {
    findActiveInstance: async () => 'inst-1' as string | null,
    findActionableTask: async (_instanceId: string, nodeId: string) =>
      nodeId === 'pns_executed' && actionable ? 'task-exec' : null,
    completeEngineTask: async () => {
      completed += 1
      actionable = false
    },
  }
  await completePnsExecutedTask('contract-1', deps)
  await completePnsExecutedTask('contract-1', deps)
  assert.equal(completed, 1)
})

test('CRM26 Contract cut: Contract workflow suppresses obsolete Deal under-contract dual-write', async () => {
  const app = createApplicationPort()
  const result = await app.executeCommand({
    commandId: 'workflow-cmd-1',
    commandType: DEAL_SET_STAGE_UNDER_CONTRACT,
    subjectType: 'contract',
    subjectId: 'contract-1',
    input: {},
  } as any)
  assert.equal(result.outcome, 'success')
  assert.match(result.message ?? '', /Contract execution already owns/)
})

test('CRM26 Contract cut E2E: Contract-subject workflow advances through pns_executed', async () => {
  const fake = new FakeSql()
  const parsed = parseReSupermodel()
  fake.seedDefinition(RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION, parsed.graph)

  const engineCalls: any[] = []
  const engine = new WorkflowEngine(fake.sql as any, {
    evaluate: stubEvaluator,
    app: makeApp({
      executeCommand: async (req: any) => {
        engineCalls.push(req)
        return { commandId: req.commandId, outcome: 'success' as const }
      },
    }) as any,
  })

  await engine.startProcess({
    definitionKey: RE_SUPERMODEL_KEY,
    startedBy: 'system',
    variables: {},
    subject: { subjectType: 'contract', subjectId: 'contract-1' },
  })

  const contract = { status: 'draft', evidenceDocumentId: null as string | null }
  const consumer = new Crm26AgreementExecutionConsumer({
    loadIssuedDocument: async () => doc(),
    executionMarkerMatches: async () => true,
    executeContract: async (_contractId, evidenceDocumentId) => {
      contract.status = 'executed'
      contract.evidenceDocumentId = evidenceDocumentId
    },
    ensureWorkflow: async () => {},
    completePnsExecuted: (contractId) =>
      completePnsExecutedTask(contractId, {
        findActiveInstance: async (id) => {
          const row = fake.store.processInstances.find(
            (instance) =>
              instance.subject_type === 'contract' &&
              instance.subject_id === id &&
              instance.status === 'active',
          )
          return row ? String(row.id) : null
        },
        findActionableTask: async (instanceId, nodeId) => {
          const row = fake.store.tasks.find(
            (task) =>
              task.process_instance_id === instanceId &&
              task.node_id === nodeId &&
              ['ready', 'reserved', 'in_progress'].includes(String(task.status)),
          )
          return row ? String(row.id) : null
        },
        completeEngineTask: async (taskId, userId, transitionName) => {
          await engine.completeTask({ taskId, userId, transitionName })
        },
      }),
  })

  await consumer.handle(message(), CONTEXT)

  assert.equal(contract.status, 'executed')
  assert.equal(contract.evidenceDocumentId, 'doc-1')
  const instance = fake.store.processInstances.find(
    (row) => row.subject_type === 'contract' && row.subject_id === 'contract-1',
  )
  assert.ok(instance, 'workflow subject is Contract')
  assert.ok(
    fake.store.tasks.some(
      (task) => task.node_id === 'pns_executed' && task.status === 'completed',
    ),
    'pns_executed completed',
  )
  assert.ok(
    engineCalls.some((call) => call.commandType === DEAL_SET_STAGE_UNDER_CONTRACT),
    'existing XML still reaches the legacy node; production application port suppresses its Deal write for Contract subjects',
  )
})
