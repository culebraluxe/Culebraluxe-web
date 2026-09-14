import { engineConfigured, engineSql } from './engine-client'
import { isUuidLike } from '../lib/deal-admin'
import { listTraceEvents } from '../db/workflow-trace'
import { sql } from '../db/client'
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
    initiatedBy: string | null
    initiatedAt: string | null
  }
  workflows: FlightRecorderWorkflow[]
  events: FlightRecorderEvent[]
  /**
   * How much of a deal's instance history this trace actually covers.
   *
   * A deal accumulates one instance per attempt, and the read caps its siblings
   * (`FLIGHT_RECORDER_SIBLING_LIMIT`) so one page view cannot become an unbounded number of trace
   * reads. A capped list that does not say it is capped is the same lie as a silent truncation, so the
   * screen is told: `{ shown, total }`, null when nothing was left out.
   */
  instances: { shown: number; total: number } | null
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
  startedBy: string | null
  definitionKey: string | null
  definitionVersion: number | null
  definitionMissing: boolean
  graph: ProcessGraph
}

async function loadInstance(
  esql: QueryExecutor,
  instanceId: string,
): Promise<LoadedInstance | null> {
  // A malformed id must never reach Postgres. `pi.id` is a uuid column, so a non-uuid string
  // raises `22P02 invalid input syntax for type uuid`, which the route could only report as a
  // generic 503 — an operator staring at a blank console with no idea that the LINK was the
  // problem. Ordinary control flow, refused at the repository boundary where the type is known.
  if (!isProcessInstanceId(instanceId)) return null

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
      pi.started_by,
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
    started_by?: unknown
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
    startedBy: r.started_by == null ? null : String(r.started_by),
    definitionKey: r.definition_key == null ? null : String(r.definition_key),
    definitionVersion: r.definition_version == null ? null : Number(r.definition_version),
    definitionMissing: r.definition == null,
    graph: def ?? { startNodeId: '', nodes: {} },
  }
}

/** All deal-scoped workflow instances for a deal (support for more than one). */
// How many sibling instances of one deal the console loads alongside the instance the operator
// opened. The engine creates an instance per attempt, so an unbounded list turns one page view
// into an unbounded number of trace reads.
const FLIGHT_RECORDER_SIBLING_LIMIT = 20

/**
 * Is this string usable as a `process_instances.id`?
 *
 * Exported because the route answers 400 (not 503) for anything else, and because the operator
 * deserves to be told that the id in the link is the problem rather than "unavailable".
 * Reuses the repository's existing uuid validator instead of a second regex dialect.
 */
export function isProcessInstanceId(value: string | null | undefined): boolean {
  return typeof value === 'string' && isUuidLike(value)
}

/** How many instances this deal has EVER had, for the truncation note. */
async function countDealInstances(
  esql: QueryExecutor,
  dealId: string,
): Promise<number> {
  if (!isProcessInstanceId(dealId)) return 0
  try {
    const rows = await esql`
      select count(*)::int as n
      from process_instances
      where subject_type = 'deal' and subject_id = ${dealId}
    `
    return Number((rows[0] as { n?: unknown } | undefined)?.n ?? 0)
  } catch {
    return 0
  }
}

async function dealScopedInstanceIds(
  esql: QueryExecutor,
  dealId: string,
): Promise<string[]> {
  if (!isProcessInstanceId(dealId)) return []
  try {
    const rows = await esql`
      select pi.id
      from process_instances pi
      where pi.subject_type = 'deal' and pi.subject_id = ${dealId}
      order by pi.created_at desc
      limit ${FLIGHT_RECORDER_SIBLING_LIMIT}
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
  // The denominator for the truncation note below. Counted, not estimated.
  const totalInstances = dealId ? await countDealInstances(esql, dealId) : instanceIds.length

  // LOAD THE INSTANCES CONCURRENTLY, IN A BOUNDED AND STABLE ORDER.
  //
  // This used to await each instance in a `for` loop, and the engine creates a new instance
  // for every attempt, so a long-lived deal accumulated dozens of them: dozens of sequential
  // full trace reads, one after another, until the platform killed the request at its function
  // timeout and the cockpit showed a 504 with nothing to explain it. The primary instance is
  // always first (it is the one the operator opened); the rest are most-recent-first.
  const ordered = [primary.id, ...instanceIds.filter((id) => id !== primary.id)]
  const loaded = await Promise.all(
    ordered.map(async (id): Promise<{ workflows: FlightRecorderWorkflow[]; events: FlightRecorderEvent[] }> => {
      const inst = id === primary.id ? primary : await loadInstance(esql, id)
      if (!inst) return { workflows: [], events: [] }
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
      return { workflows: [built.workflow], events: built.events }
    }),
  )

  const workflows: FlightRecorderWorkflow[] = []
  const events: FlightRecorderEvent[] = []
  for (const part of loaded) {
    workflows.push(...part.workflows)
    events.push(...part.events)
  }

  // Transaction context from the primary instance's subject (deal) + labels.
  const bc = await resolveBusinessContext(
    esql,
    { subjectType: primary.subjectType, subjectId: primary.subjectId },
    [],
  )

  // For a deal-scoped workflow, derive the Client from the canonical deal
  // participant model (both QA clients appear as one label) — never fabricated.
  const dealClient =
    primary.subjectType === 'deal' && primary.subjectId
      ? await resolveDealClientLabel(esql, primary.subjectId)
      : null

  const initiatedAt =
    events.length > 0 ? events.reduce((m, e) => (e.occurredAt < m ? e.occurredAt : m), events[0].occurredAt) : null

  return {
    transaction: {
      dealId: bc.dealId ?? null,
      property: bc.property ?? null,
      client: dealClient ?? bc.client ?? null,
      correlationId: primary.businessKey ?? null,
      status: primary.status ?? null,
      initiatedBy: primary.startedBy ?? null,
      initiatedAt,
    },
    workflows,
    events,
    // Measured against ALL of the deal's instances, so the screen can say "newest 20 of 34" instead
    // of presenting a capped slice as the whole history.
    instances:
      totalInstances > instanceIds.length
        ? { shown: instanceIds.length, total: totalInstances }
        : null,
  }
}

/** Resolve a deal's client participants (person display names) into one label. */
async function resolveDealClientLabel(
  esql: QueryExecutor,
  dealId: string,
): Promise<string | null> {
  try {
    const rows = await esql`
      select p.display_name as name
      from deal_participant dp
      join person p on p.id = dp.person_id
      where dp.deal_id::text = ${dealId}
        and dp.role = 'client'
        and dp.active = true
      order by p.display_name asc
    `
    const names = rows
      .map((r) => String((r as { name?: unknown }).name))
      .filter((n) => n && n !== 'undefined')
    return names.length ? names.join(' & ') : null
  } catch {
    return null
  }
}


// ---------------------------------------------------------------------------
// QA Golden transaction — durable DEV-only fixture for Product Owner QA.
// ---------------------------------------------------------------------------

/** Deterministic deal marker identifying the Flight Recorder QA golden fixture. */
export const QA_GOLDEN_DEAL_MARKER = 'qa-flight-recorder-golden'

export type GoldenQaInfo = {
  instanceId: string
  dealId: string
  property: string | null
  client: string | null
}

/**
 * Find the durable QA golden transaction's primary workflow instance (if present).
 * Returns null when the fixture has not been seeded. Used by the Flight Recorder
 * entry point to surface a one-click "Golden QA Transaction" link.
 */
export async function findGoldenQaWorkflowInstance(): Promise<GoldenQaInfo | null> {
  if (!engineConfigured()) return null
  try {
    const dealRows = await sql`
      select d.id as deal_id, d.property_id, d.client_person_id
      from deal d
      where d.notes = ${QA_GOLDEN_DEAL_MARKER}
      limit 1
    `
    if (dealRows.length === 0) return null
    const d = dealRows[0] as { deal_id: unknown; property_id: unknown; client_person_id: unknown }
    const esql = engineSql()
    const instRows = await esql`
      select pi.id
      from process_instances pi
      where pi.subject_type = 'deal' and pi.subject_id = ${String(d.deal_id)}
      order by pi.created_at asc
      limit 1
    `
    if (!instRows[0]) return null
    const instanceId = String((instRows[0] as { id: unknown }).id)
    const [property, client] = await Promise.all([
      propertyNameLabel(esql, String(d.property_id)),
      personNameLabel(esql, String(d.client_person_id)),
    ])
    return {
      instanceId,
      dealId: String(d.deal_id),
      property: property,
      client: client,
    }
  } catch {
    return null
  }
}

async function propertyNameLabel(esql: QueryExecutor, propertyId: string): Promise<string | null> {
  try {
    const rows = await esql`select name as label from property where id::text = ${propertyId} limit 1`
    const row = rows[0] as { label?: string } | undefined
    return row?.label ?? null
  } catch {
    return null
  }
}

async function personNameLabel(esql: QueryExecutor, personId: string): Promise<string | null> {
  try {
    const rows = await esql`select display_name as label from person where id::text = ${personId} limit 1`
    const row = rows[0] as { label?: string } | undefined
    return row?.label ?? null
  } catch {
    return null
  }
}
