import type { MqConsumer, MqDeliveryContext, MqMessage } from '../mq/types'
import { isExecutionEligibleTemplate } from './execution'

/** Stable production subscription identity. */
export const CRM26_SUBSCRIPTION_ID = 'crm26-agreement-execution'
export const AGREEMENT_FULLY_EXECUTED_ROUTING_KEY = 'AGREEMENT_FULLY_EXECUTED'
export const SYSTEM_ACTOR_USER_ID = 'system'

export type IssuedAgreementDocument = {
  id: string
  contractId: string | null
  templateId: string | null
  issuedVersion: number | null
}

export type CompletePnsExecutedDeps = {
  findActiveInstance: (contractId: string) => Promise<string | null>
  findActionableTask: (instanceId: string, nodeId: string) => Promise<string | null>
  completeEngineTask: (
    taskId: string,
    userId: string,
    transitionName: string,
  ) => Promise<void>
}

export type Crm26ConsumerDeps = {
  loadIssuedDocument: (documentId: string) => Promise<IssuedAgreementDocument | null>
  executionMarkerMatches: (
    documentId: string,
    issuedVersion: number,
    eventId: string,
  ) => Promise<boolean>
  executeContract: (
    contractId: string,
    evidenceDocumentId: string,
    correlationId: string,
  ) => Promise<void>
  ensureWorkflow: (contractId: string) => Promise<void>
  completePnsExecuted: (contractId: string) => Promise<void>
}

export type Crm26RejectReason =
  | 'malformed_payload'
  | 'document_not_found'
  | 'template_mismatch'
  | 'version_mismatch'
  | 'contract_mismatch'
  | 'template_not_eligible'
  | 'execution_marker_mismatch'
  | 'contract_execution_failed'
  | 'workflow_start_failed'
  | 'workflow_advance_failed'
  | 'no_active_workflow'

export class Crm26RejectError extends Error {
  readonly reason: Crm26RejectReason
  constructor(message: string, reason: Crm26RejectReason) {
    super(message)
    this.name = 'Crm26RejectError'
    this.reason = reason
  }
}

export type AgreementExecutionLocator = {
  transactionDocumentId: string
  issuedVersion: number
  templateId: string
  contractId: string
}

export function parseAgreementExecutionLocator(
  payload: Record<string, unknown>,
): AgreementExecutionLocator {
  const transactionDocumentId = payload['transactionDocumentId']
  const issuedVersion = payload['issuedVersion']
  const templateId = payload['templateId']
  const contractId = payload['contractId']
  if (
    typeof transactionDocumentId !== 'string' ||
    transactionDocumentId === '' ||
    typeof issuedVersion !== 'number' ||
    !Number.isInteger(issuedVersion) ||
    issuedVersion < 1 ||
    typeof templateId !== 'string' ||
    templateId === '' ||
    typeof contractId !== 'string' ||
    contractId === ''
  ) {
    throw new Crm26RejectError(
      'AGREEMENT_FULLY_EXECUTED payload must carry transactionDocumentId, issuedVersion, templateId and contractId.',
      'malformed_payload',
    )
  }
  return { transactionDocumentId, issuedVersion, templateId, contractId }
}

/**
 * Catch a Contract workflow up to the P&S-executed boundary.
 *
 * A fully executed P&S proves preparation happened, so if this Contract workflow
 * was started only when the execution event arrived, pns_preparation is safely
 * completed first. Replay is harmless: missing actionable tasks mean the instance
 * already advanced.
 */
export async function completePnsExecutedTask(
  contractId: string,
  deps: CompletePnsExecutedDeps,
): Promise<void> {
  const instanceId = await deps.findActiveInstance(contractId)
  if (!instanceId) {
    throw new Crm26RejectError(
      `No active RE_supermodel instance for Contract ${contractId}.`,
      'no_active_workflow',
    )
  }

  const preparationTaskId = await deps.findActionableTask(instanceId, 'pns_preparation')
  if (preparationTaskId) {
    try {
      await deps.completeEngineTask(preparationTaskId, SYSTEM_ACTOR_USER_ID, 'prepared')
    } catch (err) {
      if (!isTaskAlreadyCompleted(err)) throw err
    }
  }

  const executedTaskId = await deps.findActionableTask(instanceId, 'pns_executed')
  if (!executedTaskId) return

  try {
    await deps.completeEngineTask(executedTaskId, SYSTEM_ACTOR_USER_ID, 'executed')
  } catch (err) {
    if (isTaskAlreadyCompleted(err)) return
    throw err
  }
}

export function isTaskAlreadyCompleted(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === 'TASK_ALREADY_COMPLETED'
  )
}

export async function createCrm26Consumer(
  overrides: Partial<Crm26ConsumerDeps> = {},
): Promise<Crm26AgreementExecutionConsumer> {
  const { interactiveSql } = await import('../../lib/neon-interactive')
  const { engineConfigured, engineSql } = await import('../../workflow_app/engine-client')
  const { createApplicationPort } = await import('../../workflow_app/application-port')
  const { RESIDENTIAL_TRANSACTION_KEY } = await import('../../workflow_app/workflow-config')
  const { startResidentialContractWorkflow } = await import('../../workflow_app/runtime')
  const { ContractService, CONTRACT_OPERATIONS } = await import('../../services/contract')
  const { SqlContractRepository } = await import('../../db/contract-service-repository')

  const contractService = new ContractService(new SqlContractRepository())

  const deps: Crm26ConsumerDeps = {
    loadIssuedDocument: async (documentId) => {
      const rows = await interactiveSql`
        select id, contract_id, template_id, issued_version
        from transaction_document
        where id = ${documentId}
        limit 1
      `
      const row = rows[0] as Record<string, unknown> | undefined
      if (!row) return null
      return {
        id: String(row.id),
        contractId: row.contract_id ? String(row.contract_id) : null,
        templateId: row.template_id ? String(row.template_id) : null,
        issuedVersion: row.issued_version == null ? null : Number(row.issued_version),
      }
    },
    executionMarkerMatches: async (documentId, issuedVersion, eventId) => {
      const rows = await interactiveSql`
        select id
        from agreement_execution
        where document_id = ${documentId}
          and issued_version = ${issuedVersion}
          and event_id = ${eventId}
        limit 1
      `
      return rows.length > 0
    },
    executeContract: async (contractId, evidenceDocumentId, correlationId) => {
      const result = await contractService.execute({
        operation: CONTRACT_OPERATIONS.EXECUTE,
        payload: { contractId, evidenceDocumentId },
        context: {
          actor: { id: null, kind: 'system' },
          correlationId,
        },
      })
      if (!result.ok) {
        throw new Error(`${result.error.code}: ${result.error.message}`)
      }
    },
    ensureWorkflow: async (contractId) => {
      await startResidentialContractWorkflow(contractId)
    },
    completePnsExecuted: (contractId) =>
      completePnsExecutedTask(contractId, {
        findActiveInstance: async (id) => {
          if (!engineConfigured()) return null
          const rows = await engineSql()`
            select pi.id
            from process_instances pi
            join process_definitions pd on pd.id = pi.definition_id
            where pi.subject_type = 'contract'
              and pi.subject_id = ${id}
              and pi.status = 'active'
              and pd.key = ${RESIDENTIAL_TRANSACTION_KEY}
            limit 1
          `
          return (rows[0]?.id as string | undefined) ?? null
        },
        findActionableTask: async (instanceId, nodeId) => {
          const rows = await engineSql()`
            select id
            from tasks
            where process_instance_id = ${instanceId}
              and node_id = ${nodeId}
              and status in ('ready', 'reserved', 'in_progress')
            limit 1
          `
          return (rows[0]?.id as string | undefined) ?? null
        },
        completeEngineTask: async (taskId, userId, transitionName) => {
          const { WorkflowEngine } = await import('../../workflow_engine/lib/workflow/engine')
          const engine = new WorkflowEngine(engineSql() as never, {
            app: createApplicationPort(),
          })
          await engine.completeTask({ taskId, userId, transitionName })
        },
      }),
    ...overrides,
  }

  return new Crm26AgreementExecutionConsumer(deps)
}

export class Crm26AgreementExecutionConsumer implements MqConsumer {
  readonly subscriptionId = CRM26_SUBSCRIPTION_ID
  readonly routingKey = AGREEMENT_FULLY_EXECUTED_ROUTING_KEY
  readonly maxAttempts = 3
  readonly retryBackoffSeconds = 10

  constructor(private readonly deps: Crm26ConsumerDeps) {}

  async handle(message: MqMessage, _ctx: MqDeliveryContext): Promise<void> {
    const locator = parseAgreementExecutionLocator(message.payload)
    const doc = await this.deps.loadIssuedDocument(locator.transactionDocumentId)

    if (!doc) {
      throw new Crm26RejectError(
        `Issued transaction document ${locator.transactionDocumentId} not found.`,
        'document_not_found',
      )
    }
    if (doc.templateId !== locator.templateId) {
      throw new Crm26RejectError(
        `Lineage mismatch: event template ${locator.templateId} != document ${doc.templateId}.`,
        'template_mismatch',
      )
    }
    if (doc.issuedVersion !== locator.issuedVersion) {
      throw new Crm26RejectError(
        `Lineage mismatch: event issuedVersion ${locator.issuedVersion} != document ${doc.issuedVersion}.`,
        'version_mismatch',
      )
    }
    if (!doc.contractId || doc.contractId !== locator.contractId) {
      throw new Crm26RejectError(
        `Lineage mismatch: event Contract ${locator.contractId} != document Contract ${doc.contractId}.`,
        'contract_mismatch',
      )
    }
    if (!doc.templateId || !isExecutionEligibleTemplate(doc.templateId)) {
      throw new Crm26RejectError(
        `Template ${doc.templateId} is not an execution-eligible agreement (only PR-PNS).`,
        'template_not_eligible',
      )
    }

    const markerMatches = await this.deps.executionMarkerMatches(
      doc.id,
      doc.issuedVersion,
      message.messageId,
    )
    if (!markerMatches) {
      throw new Crm26RejectError(
        `No agreement_execution marker for ${doc.id} v${doc.issuedVersion} carrying event ${message.messageId}.`,
        'execution_marker_mismatch',
      )
    }

    const correlationId = message.correlationId ?? message.messageId
    try {
      await this.deps.executeContract(doc.contractId, doc.id, correlationId)
    } catch (err) {
      throw new Crm26RejectError(
        `Contract.execute failed for ${doc.contractId}: ${err instanceof Error ? err.message : String(err)}.`,
        'contract_execution_failed',
      )
    }

    try {
      await this.deps.ensureWorkflow(doc.contractId)
    } catch (err) {
      throw new Crm26RejectError(
        `Contract workflow start/reuse failed for ${doc.contractId}: ${err instanceof Error ? err.message : String(err)}.`,
        'workflow_start_failed',
      )
    }

    try {
      await this.deps.completePnsExecuted(doc.contractId)
    } catch (err) {
      if (err instanceof Crm26RejectError) throw err
      throw new Crm26RejectError(
        `Contract workflow advance failed for ${doc.contractId}: ${err instanceof Error ? err.message : String(err)}.`,
        'workflow_advance_failed',
      )
    }
  }
}
