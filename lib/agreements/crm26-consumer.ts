// ---------------------------------------------------------------------------
// CRM-26 — Executed P&S -> canonical Deal projection + workflow advance.
//
// A production Postgres MQ consumer for the exact AGREEMENT_FULLY_EXECUTED event
// type. On delivery it:
//
//   1. treats the event payload as a LOCATOR (not the sole truth) and reloads the
//      immutable issued transaction_document, verifying documentId / issuedVersion
//      / templateId / dealId lineage AND the agreement_execution marker/event
//      relationship;
//   2. rejects mismatched or non-PR-PNS lineage truthfully (fail closed);
//   3. maps ONLY the immutable issued source_snapshot.fieldValues through the
//      pure pns-projection mapper into existing canonical Deal commands
//      (deal.set_closing_date / set_inspection_deadline / set_financing_deadline
//      / set_financing_type / set_appraisal_required) with deterministic command
//      ids (`messageId:field`);
//   4. completes the correlated `pns_executed` workflow task through the existing
//      engine task-completion boundary (never by writing workflow tables directly,
//      never by starting a second workflow instance), then lets the existing XML
//      `mark_under_contract` command-node dispatch deal.set_stage_under_contract;
//   5. fails the MQ delivery on any projection/command failure so the existing
//      retry mechanism owns recovery (already-applied commands replay from their
//      receipts; the pns_executed task is completed only after ALL required facts
//      have succeeded or replayed).
//
// This consumer reuses existing seams only: the Postgres mini-MQ, the canonical
// command dispatcher, the workflow task-completion boundary. It creates no queue,
// no workflow, no event store, and no Deal-state model, and it never mutates the
// Deal stage directly (mark_under_contract owns that transition).
// ---------------------------------------------------------------------------

import type {
  CommandDispatcher,
  CommandEnvelope,
  CommandResult,
} from '../commands/contracts'
import type { MqConsumer, MqDeliveryContext, MqMessage } from '../mq/types'
import { isExecutionEligibleTemplate } from './execution'
import { projectPnsOperationalFields } from './pns-projection'

/** Stable, explicit production subscription identity (surfaces in mq_subscription). */
export const CRM26_SUBSCRIPTION_ID = 'crm26-agreement-execution'
/** Route ONLY this exact event type. */
export const AGREEMENT_FULLY_EXECUTED_ROUTING_KEY = 'AGREEMENT_FULLY_EXECUTED'
/** Established system-actor convention for engine task completion (not a UUID). */
export const SYSTEM_ACTOR_USER_ID = 'system'

/** Immutable issued agreement/document row the consumer reloads (not payload truth). */
export type IssuedAgreementDocument = {
  id: string
  dealId: string | null
  templateId: string | null
  issuedVersion: number | null
  sourceSnapshot: Record<string, unknown> | null
}

export type CompletePnsExecutedDeps = {
  findActiveInstance: (dealId: string) => Promise<string | null>
  findActionableTask: (instanceId: string, nodeId: string) => Promise<string | null>
  completeEngineTask: (
    taskId: string,
    userId: string,
    transitionName: string,
  ) => Promise<void>
}

export type Crm26ConsumerDeps = {
  loadIssuedDocument: (documentId: string) => Promise<IssuedAgreementDocument | null>
  /** True when an agreement_execution marker for (doc, version) carries this eventId. */
  executionMarkerMatches: (
    documentId: string,
    issuedVersion: number,
    eventId: string,
  ) => Promise<boolean>
  executeCommand: (envelope: CommandEnvelope) => Promise<CommandResult>
  completePnsExecuted: (dealId: string) => Promise<void>
}

export type Crm26RejectReason =
  | 'malformed_payload'
  | 'document_not_found'
  | 'template_mismatch'
  | 'version_mismatch'
  | 'deal_mismatch'
  | 'template_not_eligible'
  | 'execution_marker_mismatch'
  | 'deal_command_failed'
  | 'unresolved_operational_fields'
  | 'no_active_workflow'

/** A truthful, deterministic CRM-26 rejection (fail closed -> delivery fails/retries). */
export class Crm26RejectError extends Error {
  readonly reason: Crm26RejectReason
  constructor(message: string, reason: Crm26RejectReason) {
    super(message)
    this.name = 'Crm26RejectError'
    this.reason = reason
  }
}



/** Event payload locator — parsed, then verified against immutable lineage. */
export type AgreementExecutionLocator = {
  transactionDocumentId: string
  issuedVersion: number
  templateId: string
  dealId: string
}

export function parseAgreementExecutionLocator(
  payload: Record<string, unknown>,
): AgreementExecutionLocator {
  const transactionDocumentId = payload['transactionDocumentId']
  const issuedVersion = payload['issuedVersion']
  const templateId = payload['templateId']
  const dealId = payload['dealId']
  if (
    typeof transactionDocumentId !== 'string' ||
    transactionDocumentId === '' ||
    typeof issuedVersion !== 'number' ||
    !Number.isInteger(issuedVersion) ||
    issuedVersion < 1 ||
    typeof templateId !== 'string' ||
    templateId === '' ||
    typeof dealId !== 'string' ||
    dealId === ''
  ) {
    throw new Crm26RejectError(
      'AGREEMENT_FULLY_EXECUTED payload must carry transactionDocumentId, issuedVersion, templateId and dealId.',
      'malformed_payload',
    )
  }
  return { transactionDocumentId, issuedVersion, templateId, dealId }
}



/** Build the canonical command envelope for one projection (correlation preserved). */
export function buildProjectionCommandEnvelope(
  projection: {
    commandType: string
    commandId: string
    aggregateId: string
    input: Record<string, unknown>
  },
  message: MqMessage,
): CommandEnvelope {
  return {
    commandId: projection.commandId,
    commandType: projection.commandType,
    actorAppUserId: null, // system actor: no app_user; the receipt records a null actor
    aggregateType: 'deal',
    aggregateId: projection.aggregateId,
    correlationId: message.correlationId ?? message.messageId,
    causationId: message.messageId,
    requestedAt: message.occurredAt,
    input: projection.input,
  }
}


/**
 * Complete the correlated `pns_executed` engine task for a deal's ACTIVE
 * RE_supermodel instance through the established engine task-completion boundary.
 *
 * Reuses the existing active instance (never starts a second one). If the task is
 * already completed / already advanced past pns_executed, that is successful
 * recovery — not a reason to create a new instance or task.
 */
export async function completePnsExecutedTask(
  dealId: string,
  deps: CompletePnsExecutedDeps,
): Promise<void> {
  const instanceId = await deps.findActiveInstance(dealId)
  if (!instanceId) {
    throw new Crm26RejectError(
      `No active RE_supermodel instance for deal ${dealId} to advance pns_executed.`,
      'no_active_workflow',
    )
  }
  const taskId = await deps.findActionableTask(instanceId, 'pns_executed')
  if (!taskId) {
    // Already advanced past (or already completed) pns_executed — recovery success.
    return
  }
  try {
    await deps.completeEngineTask(taskId, SYSTEM_ACTOR_USER_ID, 'executed')
  } catch (err) {
    if (isTaskAlreadyCompleted(err)) return // replay-safe recovery
    throw err
  }
}

/** The engine's deterministic duplicate-completion conflict code. */
export function isTaskAlreadyCompleted(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === 'TASK_ALREADY_COMPLETED'
  )
}

/**
 * Build the production CRM-26 consumer with the real Postgres / engine wiring.
 * DB + engine seams are imported lazily so importing this module never requires a
 * DATABASE_URL. `overrides` let tests inject fakes for every seam.
 */
export async function createCrm26Consumer(
  overrides: Partial<Crm26ConsumerDeps> = {},
): Promise<Crm26AgreementExecutionConsumer> {
  const { interactiveSql } = await import('../../lib/neon-interactive')
  const { createCommandDispatcher } = await import('../commands')
  const { engineConfigured, engineSql } = await import('../../workflow_app/engine-client')
  const { createApplicationPort } = await import('../../workflow_app/application-port')
  const { RESIDENTIAL_TRANSACTION_KEY } = await import('../../workflow_app/workflow-config')

  const dispatcher = createCommandDispatcher()

  const deps: Crm26ConsumerDeps = {
    loadIssuedDocument: async (documentId) => {
      const rows = await interactiveSql`
        select id, deal_id, template_id, issued_version, source_snapshot
        from transaction_document
        where id = ${documentId}
        limit 1
      `
      const row = rows[0] as Record<string, unknown> | undefined
      if (!row) return null
      return {
        id: String(row.id),
        dealId: row.deal_id ? String(row.deal_id) : null,
        templateId: row.template_id ? String(row.template_id) : null,
        issuedVersion: row.issued_version == null ? null : Number(row.issued_version),
        sourceSnapshot: (row.source_snapshot as Record<string, unknown> | null) ?? null,
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
    executeCommand: async (envelope) => dispatcher.execute(envelope),
    completePnsExecuted: (dealId) =>
      completePnsExecutedTask(dealId, {
        findActiveInstance: async (id) => {
          if (!engineConfigured()) return null
          const rows = await engineSql()`
            select pi.id
            from process_instances pi
            join process_definitions pd on pd.id = pi.definition_id
            where pi.subject_type = 'deal'
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

    // 2. Reload the immutable issued document and verify lineage truthfully.
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
    if (!doc.dealId || doc.dealId !== locator.dealId) {
      throw new Crm26RejectError(
        `Lineage mismatch: event deal ${locator.dealId} != document deal ${doc.dealId}.`,
        'deal_mismatch',
      )
    }
    if (!doc.templateId || !isExecutionEligibleTemplate(doc.templateId)) {
      throw new Crm26RejectError(
        `Template ${doc.templateId} is not an execution-eligible agreement (only PR-PNS).`,
        'template_not_eligible',
      )
    }

    // Verify the agreement_execution marker/event relationship (exactly-once fact).
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

    // 3. Project ONLY the immutable issued field values into canonical Deal facts.
    const fieldValues = (doc.sourceSnapshot as Record<string, unknown> | null)?.fieldValues
    const outcome = projectPnsOperationalFields({
      dealId: doc.dealId,
      sourceId: message.messageId,
      fieldValues:
        fieldValues && typeof fieldValues === 'object'
          ? (fieldValues as Record<string, unknown>)
          : {},
    })

    // 4. Execute the canonical Deal commands idempotently (deterministic command ids).
    for (const projection of outcome.projections) {
      const envelope = buildProjectionCommandEnvelope(projection, message)
      const result = await this.deps.executeCommand(envelope)
      if (result.outcome !== 'success') {
        throw new Crm26RejectError(
          `Deal command ${projection.commandType} for deal ${doc.dealId} failed: ${result.message ?? result.outcome}.`,
          'deal_command_failed',
        )
      }
    }

    // 5. A present-but-unresolved field stays visibly unresolved: fail the delivery
    //    (broker retries then dead-letters with this error) and do NOT advance the
    //    workflow, so the deal remains visibly awaiting review.
    if (outcome.unresolved.length > 0) {
      const detail = outcome.unresolved
        .map((u) => `${u.field} (${u.reason})`)
        .join(', ')
      throw new Crm26RejectError(
        `PR-PNS fields remain unresolved and require review: ${detail}.`,
        'unresolved_operational_fields',
      )
    }

    // 6. Complete the correlated pns_executed task ONLY after all facts succeeded
    //    or replayed. The existing mark_under_contract command-node then dispatches
    //    deal.set_stage_under_contract through the canonical dispatcher.
    await this.deps.completePnsExecuted(doc.dealId)
  }
}


