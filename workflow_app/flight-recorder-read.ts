import { engineConfigured, engineSql } from './engine-client'
import { listTraceEvents } from '../db/workflow-trace'
import { buildRuntimeInspection } from '../lib/runtime-inspector'
import type { NodeRuntime } from '../lib/runtime-inspector'
import { resolveBusinessContext } from './runtime-inspector-read'
import type { ProcessGraph } from '../workflow_engine/lib/workflow/types'
import type { TraceEvent } from '../lib/workflow-trace'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER — the canonical server-side read model for the Flight Recorder
// / Grok view.
//
// It joins ONE business transaction to its workflow instance(s), their EXACT
// persisted workflow definitions, and the real durable trace evidence — then maps
// each trace event onto the matching node of that instance's definition.
//
// Responsibilities are kept separate (the work-order identity rules):
//   - transaction context (deal / property / client)
//   - workflow instance(s)  -> exact persisted definition (historical)
//   - real trace events     -> mappedWorkflowNode (by immutable workflowNodeId)
//
// This REUSES the existing shared graph interpreter (buildRuntimeInspection) —
// it is NOT a second workflow parser.
// ---------------------------------------------------------------------------

export type FlightRecorderMappedNode = {
  id: string
  name: string | null
  type: string | null
  description: string | null
}

export type FlightRecorderEvent = {
  eventId: string
  occurredAt: string
  eventType: string
  sourceSystem: string
  summary: string | null
  outcome: string | null
  durationMs: number | null
  traceId: string | null
  correlationId: string | null
  workflowInstanceId: string | null
  workflowNodeId: string | null
  causationId: string | null
  commandId: string | null
  domainEventId: string | null
  documentId: string | null
  signatureRequestId: string | null
  metadata: Record<string, unknown> | null
  mappedWorkflowNode: FlightRecorderMappedNode | null
}

export type FlightRecorderWorkflow = {
  workflowInstanceId: string
  definitionId: string | null
  definitionKey: string | null
  definitionVersion: number | null
  /** True when the instance exists but its persisted definition is missing. */
  definitionMissing: boolean
  status: string | null
  currentNodeId: string | null
  graph: ProcessGraph
  nodeStates: Record<string, NodeRuntime>
}

export type FlightRecorderTransaction = {
  transaction: {
    dealId: string | null
    property: string | null
    client: string | null
    correlationId: string | null
    status: string | null
  }
  workflows: FlightRecorderWorkflow[]
  events: FlightRecorderEvent[]
}

// ---------------------------------------------------------------------------
// Pure mapping helpers (unit-testable without a database).
// ---------------------------------------------------------------------------

/**
 * Map a trace event to its node inside the instance's exact persisted definition.
 * The workflow_node_id relationship is authoritative; no name/event lookup table.
 */
export function mapEventToWorkflowNode(
  e: TraceEvent,
  graph: ProcessGraph,
): FlightRecorderMappedNode | null {
  if (!e.workflowNodeId) return null
  const node = graph.nodes[e.workflowNodeId]
  if (!node) return null
  return {
    id: node.id,
    name: node.name ?? null,
    type: node.type ?? null,
    description: node.description ?? null,
  }
}

export function toFlightRecorderEvent(
  e: TraceEvent,
  graph: ProcessGraph,
): FlightRecorderEvent {
  return {
    eventId: e.id ?? e.sourceEventId ?? e.correlationId ?? 'evt',
    occurredAt: e.occurredAt,
    eventType: e.eventType,
    sourceSystem: e.system,
    summary: e.summary,
    outcome: e.outcome,
    durationMs: e.durationMs,
    traceId: e.traceId ?? null,
    correlationId: e.correlationId ?? null,
    workflowInstanceId: e.workflowInstanceId ?? null,
    workflowNodeId: e.workflowNodeId ?? null,
    causationId: e.causationId ?? null,
    commandId: e.commandId ?? null,
    domainEventId: e.domainEventId ?? null,
    documentId: e.transactionDocumentId ?? null,
    signatureRequestId: e.signatureRequestId ?? null,
    metadata: e.metadata ?? null,
    mappedWorkflowNode: mapEventToWorkflowNode(e, graph),
  }
}

export type BuildWorkflowInput = {
  workflowInstanceId: string
  definitionId: string | null
  definitionKey: string | null
  definitionVersion: number | null
  definitionMissing: boolean
  status: string | null
  graph: ProcessGraph
  events: TraceEvent[]
}

/**
 * Build ONE workflow's read-model entry: exact definition graph, semantic node
 * states (derived via the shared buildRuntimeInspection), and events mapped to
 * workflow nodes. Node states drive the Trace Map; events are kept even when
 * they have no workflow_node_id (supporting evidence).
 */
export function buildFlightRecorderWorkflow(
  input: BuildWorkflowInput,
): { workflow: FlightRecorderWorkflow; events: FlightRecorderEvent[] } {
  const inspection = buildRuntimeInspection(
    input.workflowInstanceId,
    input.graph,
    input.events,
  )
  const nodeStates: Record<string, NodeRuntime> = {}
  for (const n of inspection.nodes) nodeStates[n.nodeId] = n

  return {
    workflow: {
      workflowInstanceId: input.workflowInstanceId,
      definitionId: input.definitionId,
      definitionKey: input.definitionKey,
      definitionVersion: input.definitionVersion,
      definitionMissing: input.definitionMissing,
      status: input.status,
      currentNodeId: inspection.currentNodeId,
      graph: input.graph,
      nodeStates,
    },
    events: input.events.map((e) => toFlightRecorderEvent(e, input.graph)),
  }
}
// ---------------------------------------------------------------------------
// Database loading.
// ---------------------------------------------------------------------------

type LoadedInstance = {
  id: string
  definitionId: string | null
  status: string | null
  subjectType: string | null
  subjectId: string | null
  businessKey: string | null
  definitionKey: string | null
  definitionVersion: number | null
  definitionMissing: boolean
  graph: ProcessGraph
}

async function loadInstance(
  esql: QueryExecutor,
  instanceId: string,
): Promise<LoadedInstance | null> {
  // LEFT JOIN so an instance whose persisted definition is missing is still
  // returned as a diagnostic (we never silently substitute another version).
  const rows = await esql`
    select
      pi.id as instance_id,
      pi.definition_id,
      pi.status,
      pi.subject_type,
      pi.subject_id,
      pi.business_key,
      pd.key as definition_key,
      pd.version as definition_version,
      pd.definition
    from process_instances pi
    left join process_definitions pd on pd.id = pi.definition_id
    where pi.id = ${instanceId}
    limit 1
  `
  const r = rows[0] as {
    instance_id?: unknown
    definition_id?: unknown
    status?: unknown
    subject_type?: unknown
    subject_id?: unknown
    business_key?: unknown
    definition_key?: unknown
    definition_version?: unknown
    definition?: unknown
  } | undefined
  if (r?.instance_id == null) return null
  const def = r.definition as ProcessGraph | undefined
  return {
    id: String(r.instance_id),
    definitionId: r.definition_id == null ? null : String(r.definition_id),
    status: r.status == null ? null : String(r.status),
    subjectType: r.subject_type == null ? null : String(r.subject_type),
    subjectId: r.subject_id == null ? null : String(r.subject_id),
    businessKey: r.business_key == null ? null : String(r.business_key),
    definitionKey: r.definition_key == null ? null : String(r.definition_key),
    definitionVersion: r.definition_version == null ? null : Number(r.definition_version),
    definitionMissing: r.definition == null,
    graph: def ?? { startNodeId: '', nodes: {} },
  }
}

/** All deal-scoped workflow instances for a deal (support for more than one). */
async function dealScopedInstanceIds(
  esql: QueryExecutor,
  dealId: string,
): Promise<string[]> {
  try {
    const rows = await esql`
      select pi.id
      from process_instances pi
      where pi.subject_type = 'deal' and pi.subject_id = ${dealId}
      order by pi.created_at asc
    `
    return rows
      .map((r) => String((r as { id?: unknown }).id))
      .filter((v) => v && v !== 'undefined')
  } catch {
    return []
  }
}

/**
 * Load the Flight Recorder transaction read model for a workflow instance. The
 * primary instance's deal (subject) resolves sibling workflow instances, so the
 * model never assumes one workflow per deal. Historical fidelity is guaranteed:
 * each instance loads ITS OWN process_definition row by definition_id.
 */
export async function getFlightRecorderTransaction(
  instanceId: string,
): Promise<FlightRecorderTransaction | null> {
  if (!engineConfigured()) return null
  const esql = engineSql()

  const primary = await loadInstance(esql, instanceId)
  if (!primary) return null

  const dealId = primary.subjectType === 'deal' ? primary.subjectId : null
  const siblingIds = dealId ? await dealScopedInstanceIds(esql, dealId) : []
  const instanceIds = Array.from(new Set([primary.id, ...siblingIds]))

  const workflows: FlightRecorderWorkflow[] = []
  const events: FlightRecorderEvent[] = []
  for (const id of instanceIds) {
    const inst = await loadInstance(esql, id)
    if (!inst) continue
    const trace = await listTraceEvents({ workflowInstanceId: id })
    const built = buildFlightRecorderWorkflow({
      workflowInstanceId: id,
      definitionId: inst.definitionId,
      definitionKey: inst.definitionKey,
      definitionVersion: inst.definitionVersion,
      definitionMissing: inst.definitionMissing,
      status: inst.status,
      graph: inst.graph,
      events: trace,
    })
    workflows.push(built.workflow)
    events.push(...built.events)
  }

  // Transaction context from the primary instance's subject (deal) + labels.
  const bc = await resolveBusinessContext(
    esql,
    { subjectType: primary.subjectType, subjectId: primary.subjectId },
    [],
  )

  return {
    transaction: {
      dealId: bc.dealId ?? null,
      property: bc.property ?? null,
      client: bc.client ?? null,
      correlationId: primary.businessKey ?? null,
      status: primary.status ?? null,
    },
    workflows,
    events,
  }
}

