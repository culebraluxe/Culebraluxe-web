import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { sanitizeMetadata, type TraceEvent } from '../lib/workflow-trace'

// ---------------------------------------------------------------------------
// WORKFLOW-TRACE — durable Flight Recorder seam (observer-only).
//
// The recorder writes immutable trace evidence and is COMPLETELY
// NON-AUTHORITATIVE. No application behavior may depend on recorder success:
// record() swallows every failure (logs a bounded incident) and NEVER throws,
// so a recorder outage can never break a workflow, command, task, timer,
// signature, or UI read. This honors the existing DatabaseGateway containment
// architecture — we never bypass or weaken it, we simply catch at the recorder
// boundary so the caller is never coupled to tracing.
// ---------------------------------------------------------------------------

export type RecordTraceInput = Partial<TraceEvent> & {
  eventType: TraceEvent['eventType']
  system: string
  occurredAt: string
}

export type TraceFilter = {
  workflowInstanceId?: string | null
  dealId?: string | null
  traceId?: string | null
  correlationId?: string | null
  limit?: number
}

const DEFAULT_SOURCE_SYSTEM = 'workflow_runtime'

/**
 * Persist one trace event, replay-safe and observer-only. Sanitizes metadata,
 * de-dupes on (source_system, source_event_id) when a source identity is
 * supplied, and NEVER throws — recorder failure is contained and logged.
 */
export async function recordTraceEvent(
  input: RecordTraceInput,
  execute: QueryExecutor = sql,
): Promise<void> {
  try {
    const metadata = sanitizeMetadata(input.metadata)
    await execute`
      insert into workflow_execution_trace_event (
        trace_id, correlation_id, causation_id,
        deal_id, person_id, property_id, transaction_document_id,
        workflow_instance_id, workflow_definition_key, workflow_definition_version,
        workflow_node_id, workflow_transition_id,
        event_type, system, occurred_at, completed_at, duration_ms, outcome,
        command_id, domain_event_id, task_id, timer_job_id, signature_request_id, external_reference,
        summary, metadata, source_system, source_event_id
      ) values (
        ${input.traceId ?? null}, ${input.correlationId ?? null}, ${input.causationId ?? null},
        ${input.dealId ?? null}, ${input.personId ?? null}, ${input.propertyId ?? null}, ${input.transactionDocumentId ?? null},
        ${input.workflowInstanceId ?? null}, ${input.workflowDefinitionKey ?? null}, ${input.workflowDefinitionVersion ?? null},
        ${input.workflowNodeId ?? null}, ${input.workflowTransitionId ?? null},
        ${input.eventType}, ${input.system}, ${input.occurredAt}, ${input.completedAt ?? null}, ${input.durationMs ?? null}, ${input.outcome ?? null},
        ${input.commandId ?? null}, ${input.domainEventId ?? null}, ${input.taskId ?? null}, ${input.timerJobId ?? null}, ${input.signatureRequestId ?? null}, ${input.externalReference ?? null},
        ${input.summary ?? null}, ${metadata}, ${input.sourceSystem ?? DEFAULT_SOURCE_SYSTEM}, ${input.sourceEventId ?? null}
      )
      on conflict (source_system, source_event_id) where source_event_id is not null do nothing
    `
  } catch (err) {
    // Observer-only: a recorder failure must never break the business operation.
    // Log a bounded incident (no raw SQL, no credentials) and continue.
    const message = err instanceof Error ? err.message : String(err)
    // eslint-disable-next-line no-console
    console.error(`[workflow-trace] recorder write failed (contained): ${message.slice(0, 200)}`)
  }
}

/** Convenience observer-only recorder facade for instrumentation callers. */
export const flightRecorder = {
  record: (input: RecordTraceInput, execute?: QueryExecutor) =>
    recordTraceEvent(input, execute),
}

// ---------------------------------------------------------------------------
// Runtime Inspector reads (reconstruction happens here, not on the write path).
// ---------------------------------------------------------------------------

export async function listTraceEvents(
  filter: TraceFilter,
  execute: QueryExecutor = sql,
): Promise<TraceEvent[]> {
  const limit = Math.min(5000, filter.limit ?? 1000)
  // Fully-parameterized null-guarded filters — works with any QueryExecutor
  // (the gateway and the pool executor both parameterize interpolated values,
  // so we never inject a raw dynamic WHERE string).
  const rows = (await execute`
    select * from workflow_execution_trace_event
    where (${filter.workflowInstanceId ?? null}::text is null or workflow_instance_id = ${filter.workflowInstanceId ?? null})
      and (${filter.dealId ?? null}::text is null or deal_id = ${filter.dealId ?? null})
      and (${filter.traceId ?? null}::text is null or trace_id = ${filter.traceId ?? null})
      and (${filter.correlationId ?? null}::text is null or correlation_id = ${filter.correlationId ?? null})
    order by occurred_at asc
    limit ${limit}
  `) as unknown as Array<Record<string, unknown>>

  return rows.map((r) => rowToTraceEvent(r))
}

function rowToTraceEvent(r: Record<string, unknown>): TraceEvent {
  return {
    id: str(r.id),
    eventType: String(r.event_type),
    system: String(r.system),
    occurredAt: toIso(r.occurred_at as string | Date),
    completedAt: r.completed_at == null ? null : toIso(r.completed_at as string | Date),
    durationMs: r.duration_ms == null ? null : Number(r.duration_ms),
    outcome: r.outcome == null ? null : String(r.outcome),
    traceId: str(r.trace_id),
    correlationId: str(r.correlation_id),
    causationId: str(r.causation_id),
    dealId: str(r.deal_id),
    personId: str(r.person_id),
    propertyId: str(r.property_id),
    transactionDocumentId: str(r.transaction_document_id),
    workflowInstanceId: str(r.workflow_instance_id),
    workflowDefinitionKey: str(r.workflow_definition_key),
    workflowDefinitionVersion: r.workflow_definition_version == null ? null : Number(r.workflow_definition_version),
    workflowNodeId: str(r.workflow_node_id),
    workflowTransitionId: str(r.workflow_transition_id),
    commandId: str(r.command_id),
    domainEventId: str(r.domain_event_id),
    taskId: str(r.task_id),
    timerJobId: str(r.timer_job_id),
    signatureRequestId: str(r.signature_request_id),
    externalReference: str(r.external_reference),
    summary: str(r.summary),
    metadata: (r.metadata as Record<string, unknown> | null) ?? null,
    sourceSystem: str(r.source_system),
    sourceEventId: str(r.source_event_id),
  }
}

function str(value: unknown): string | null {
  return value == null ? null : String(value)
}

function toIso(value: string | Date): string {
  const d = value instanceof Date ? value : new Date(value)
  return Number.isNaN(d.getTime()) ? String(value) : d.toISOString()
}

