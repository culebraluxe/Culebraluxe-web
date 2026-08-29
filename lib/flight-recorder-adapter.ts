// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-ADAPTER — bridge the Runtime Inspector (real engine evidence)
// into the Flight Recorder console read-model.
//
// The Runtime Inspector reconstructs execution evidence from immutable trace
// events over the design-time topology (`RuntimeInspection`). The Flight
// Recorder console is the facelift UI built against a richer "correlation"
// read-model (`FlightRecorderTrace`). This module is the PURE, DETERMINISTIC
// adapter between the two: it never touches the DB and never executes business
// logic. Given the same inspection it always produces the same console trace,
// which keeps the console honest and testable against real engine data.
//
// HONEST DEGRADATION: the real trace stores bounded, sanitized `metadata` (no
// full business payloads) and the engine writes a small closed `system`
// vocabulary ('command' | 'domain' | 'workflow'). The adapter maps those onto
// the console's richer enums with stable fallbacks and exposes `metadata` as
// the "payload". It never fabricates business facts that were not recorded.
// ---------------------------------------------------------------------------

import type { RuntimeInspection, TimelineEntry, NodeRuntimeState } from './runtime-inspector'
import type {
  FlightRecorderTransaction,
  FlightRecorderEvent,
  FlightRecorderWorkflow,
} from '../workflow_app/flight-recorder-read'

// ---------------------------------------------------------------------------
// Console read-model (the contract the Flight Recorder console renders).
// ---------------------------------------------------------------------------

export type EventKind =
  | 'Command'
  | 'DomainEvent'
  | 'Workflow'
  | 'Task'
  | 'Integration'
  | 'Persistence'
  | 'Unknown'

export type SystemId =
  | 'API Gateway'
  | 'Domain Model'
  | 'Workflow Engine'
  | 'Task Service'
  | 'BoldSign'
  | 'PostgreSQL'
  | 'Unknown'

export type EventStatus = 'Success' | 'Failed' | 'Pending' | 'Skipped' | 'Unknown'

export interface RelatedEventRef {
  id: string
  title: string
  /** Signed offset from the selected event, in milliseconds. */
  offsetMs: number
}

export interface TraceEvent {
  id: string
  correlationId: string
  causationId: string | null
  parentIds?: string[]
  /** Stable identity projections carried through so causal grouping stays exact. */
  commandId: string | null
  domainEventId: string | null
  /** The workflow node this event maps to (immutable workflowNodeId), if any. */
  workflowNodeId?: string | null
  kind: EventKind
  type: string
  title: string
  subtitle: string
  system: SystemId
  status: EventStatus
  occurredAt: string
  offsetMs: number
  durationMs: number
  details: Record<string, string>
  payload: unknown
  tags: string[]
  relatedEventIds: RelatedEventRef[]
}

export interface BusinessContext {
  dealId?: string
  /** Resolved human-readable deal label (falls back to the deal id). */
  deal?: string
  property?: string
  client?: string
  workflow?: string
  initiatedBy?: string
  initiatedAt?: string
  [k: string]: string | undefined
}

export interface TraceSummary {
  correlationId: string
  rootTitle: string
  rootKind: EventKind
  durationMs: number
  eventCount: number
  systemCount: number
  status: 'Completed' | 'Failed' | 'InProgress'
  businessContext: BusinessContext
}

export interface FlightRecorderTrace {
  summary: TraceSummary
  events: TraceEvent[]
  /**
   * The EXACT persisted master-workflow graph for the primary workflow instance,
   * with per-node execution state. Drives the Trace Map (expected process
   * topology) — NOT the causal event graph.
   */
  workflow?: ConsoleWorkflowView
}

export type ConsoleWorkflowNode = {
  id: string
  name: string
  type: string
  /** Grok semantic identity derived from the workflow node type (never the name). */
  semanticKind: EventKind
  state: NodeRuntimeState
}

export type ConsoleWorkflowTransition = {
  from: string
  to: string
  name: string
}

export type ConsoleWorkflowView = {
  workflowInstanceId: string
  definitionKey: string | null
  definitionVersion: number | null
  currentNodeId: string | null
  nodes: ConsoleWorkflowNode[]
  transitions: ConsoleWorkflowTransition[]
}

export type TimelineDensity = 'compact' | 'expanded'
export type MainTab = 'timeline' | 'causality' | 'swimlane' | 'raw'

// Kind → color token (presentation; kept with the domain so the console and any
// graph overlay stay consistent).
export const KIND_TOKENS: Record<
  EventKind,
  { bg: string; text: string; chip: string; label: string }
> = {
  Command: {
    bg: 'bg-violet-500',
    text: 'text-violet-300',
    chip: 'bg-violet-500/20 text-violet-200 ring-1 ring-violet-400/30',
    label: 'Command',
  },
  DomainEvent: {
    bg: 'bg-sky-500',
    text: 'text-sky-300',
    chip: 'bg-sky-500/20 text-sky-200 ring-1 ring-sky-400/30',
    label: 'Domain Event',
  },
  Workflow: {
    bg: 'bg-emerald-500',
    text: 'text-emerald-300',
    chip: 'bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30',
    label: 'Workflow',
  },
  Task: {
    bg: 'bg-amber-500',
    text: 'text-amber-300',
    chip: 'bg-amber-500/20 text-amber-200 ring-1 ring-amber-400/30',
    label: 'Task',
  },
  Integration: {
    bg: 'bg-fuchsia-500',
    text: 'text-fuchsia-300',
    chip: 'bg-fuchsia-500/20 text-fuchsia-200 ring-1 ring-fuchsia-400/30',
    label: 'Integration',
  },
  Persistence: {
    bg: 'bg-teal-500',
    text: 'text-teal-300',
    chip: 'bg-teal-500/20 text-teal-200 ring-1 ring-teal-400/30',
    label: 'Persistence',
  },
  Unknown: {
    bg: 'bg-slate-500',
    text: 'text-slate-300',
    chip: 'bg-slate-500/20 text-slate-200 ring-1 ring-slate-400/30',
    label: 'Unknown',
  },
}

export const SYSTEM_CHIP: Record<SystemId, string> = {
  'API Gateway': 'bg-indigo-500/20 text-indigo-200 ring-1 ring-indigo-400/30',
  'Domain Model': 'bg-blue-500/20 text-blue-200 ring-1 ring-blue-400/30',
  'Workflow Engine': 'bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30',
  'Task Service': 'bg-amber-600/20 text-amber-200 ring-1 ring-amber-400/30',
  BoldSign: 'bg-pink-500/20 text-pink-200 ring-1 ring-pink-400/30',
  PostgreSQL: 'bg-cyan-500/20 text-cyan-200 ring-1 ring-cyan-400/30',
  Unknown: 'bg-slate-500/20 text-slate-200 ring-1 ring-slate-400/30',
}

// ---------------------------------------------------------------------------
// Classification helpers (pure).
// ---------------------------------------------------------------------------

/** Map an engine eventType onto the console's EventKind vocabulary. */
export function eventTypeToKind(eventType: string): EventKind {
  const u = eventType.toUpperCase()
  if (u.startsWith('COMMAND_')) return 'Command'
  if (u.startsWith('DOMAIN_EVENT')) return 'DomainEvent'
  if (u.startsWith('WORKFLOW_') || u.startsWith('NODE_') || u.startsWith('TRANSITION_'))
    return 'Workflow'
  if (u.startsWith('TASK_') || u.startsWith('TIMER_') || u.startsWith('JOB_')) return 'Task'
  if (u.startsWith('SIGNATURE_')) return 'Integration'
  if (u.startsWith('DOCUMENT_')) return 'Persistence'
  // Truthful fallback: an unrecognized event is NOT forced into a known domain.
  return 'Unknown'
}

/**
 * Map the engine's open-ended `system` string onto the console's SystemId enum.
 * A named subsystem is assigned ONLY when the raw producer string itself supports
 * it. Unknown producers stay 'Unknown' — the raw value is preserved separately
 * on the event so the UI never guesses a provider (no automatic BoldSign /
 * PostgreSQL / API Gateway / Workflow Engine for unknown evidence).
 */
export function systemToSystemId(system: string, _kind: EventKind): SystemId {
  const s = system.toLowerCase()
  if (s.includes('domain')) return 'Domain Model'
  if (s.includes('command') || s.includes('api') || s.includes('gateway')) return 'API Gateway'
  if (s.includes('workflow')) return 'Workflow Engine'
  if (s.includes('task')) return 'Task Service'
  if (s.includes('boldsign') || s.includes('signature')) return 'BoldSign'
  if (s.includes('postgres') || s.includes('sql') || s.includes('persist')) return 'PostgreSQL'
  return 'Unknown'
}

/** Map an engine outcome onto the console's EventStatus. */
export function outcomeToStatus(
  outcome: string | null,
  eventType: string,
): EventStatus {
  const o = (outcome ?? '').toUpperCase()
  if (o === 'FAILURE' || o === 'FAILED' || o === 'ERROR') return 'Failed'
  if (o === 'STARTED' || o === 'PENDING') return 'Pending'
  if (o === 'SUCCESS' || o === 'COMPLETED' || o === 'RECOVERED' || o === 'REPLAYED')
    return 'Success'
  const u = eventType.toUpperCase()
  if (u === 'FAILURE' || u === 'RETRY' || u.endsWith('_FAILED')) return 'Failed'
  // Entry / await states read as pending when no explicit outcome was recorded.
  if (
    u.endsWith('_STARTED') ||
    u === 'NODE_ENTERED' ||
    u === 'WORKFLOW_STARTED' ||
    u === 'COMMAND_RECEIVED' ||
    u === 'COMMAND_REPLAYED' ||
    u === 'TASK_CREATED' ||
    u === 'TASK_ASSIGNED' ||
    u === 'TIMER_SCHEDULED' ||
    u === 'JOB_STARTED' ||
    u === 'SIGNATURE_REQUEST_CREATED' ||
    u === 'SIGNATURE_SENT' ||
    u === 'DOCUMENT_CREATED'
  )
    return 'Pending'
  // Absence of failure is NOT proof of success.
  return 'Unknown'
}

/**
 * Map a master-workflow NodeDefinition.type onto a Grok semantic EventKind so
 * the Master Workflow view reuses the same visual language as the Timeline.
 * The mapping is CENTRAL here (presentation transform), driven by actual node
 * semantics, never by display name. Unrecognized types stay 'Unknown'.
 */
export function nodeTypeToKind(nodeType: string): EventKind {
  const t = nodeType.trim().toLowerCase()
  if (t === 'command') return 'Command'
  if (t === 'state' || t === 'domain' || t === 'domain_event') return 'DomainEvent'
  if (t === 'task' || t === 'user_task' || t === 'human') return 'Task'
  if (t === 'integration' || t === 'external' || t === 'provider' || t === 'signature')
    return 'Integration'
  if (t === 'persistence' || t === 'document' || t === 'storage') return 'Persistence'
  if (
    t === 'start' ||
    t === 'end' ||
    t === 'decision' ||
    t === 'fork' ||
    t === 'join' ||
    t === 'timer' ||
    t === 'subprocess' ||
    t === 'workflow'
  )
    return 'Workflow'
  return 'Unknown'
}

function humanize(s: string): string {
  return s
    .toLowerCase()
    .split('_')
    .filter(Boolean)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

// ---------------------------------------------------------------------------
// Causation resolution.
// ---------------------------------------------------------------------------

type IdIndex = {
  byId: Map<string, TimelineEntry>
  byCommand: Map<string, string>
  byDomain: Map<string, string>
}

function buildIdIndex(timeline: TimelineEntry[]): IdIndex {
  const byId = new Map<string, TimelineEntry>()
  const byCommand = new Map<string, string>()
  const byDomain = new Map<string, string>()
  for (const e of timeline) {
    byId.set(e.id, e)
    // A DOMAIN_EVENT_EMITTED row's commandId is the CAUSING command (a parent
    // reference), not its own identity, so it must not be indexed as a command
    // node — otherwise the domain event resolves to itself.
    if (e.commandId && e.eventType !== 'DOMAIN_EVENT_EMITTED') {
      byCommand.set(e.commandId, e.id)
    }
    if (e.domainEventId) byDomain.set(e.domainEventId, e.id)
  }
  return { byId, byCommand, byDomain }
}

/** Resolve a raw causation reference to a console event id (best effort). */
function resolveCausation(raw: string | null, idx: IdIndex): string | null {
  if (!raw) return null
  if (idx.byId.has(raw)) return raw
  return idx.byCommand.get(raw) ?? idx.byDomain.get(raw) ?? null
}

// ---------------------------------------------------------------------------
// The adapter.
// ---------------------------------------------------------------------------

/** Read-side resolved business-context labels merged into the console summary. */
export type ResolvedConsoleContext = {
  dealId?: string | null
  propertyId?: string | null
  personId?: string | null
  deal?: string | null
  property?: string | null
  client?: string | null
}

export type AdapterInput = {
  inspection: RuntimeInspection
  nodeTypes?: Record<string, string>
  /** Optional read-side resolved business-context labels (real deal/property/client). */
  resolvedBusinessContext?: ResolvedConsoleContext
}

export function adaptRuntimeInspection(input: AdapterInput): FlightRecorderTrace {
  const { inspection, resolvedBusinessContext } = input
  const timeline = inspection.timeline
  const correlationId = inspection.workflowInstanceId

  const idx = buildIdIndex(timeline)

  // Pre-resolve causation for every event so children lookups are cheap and
  // deterministic.
  const resolved = new Map<string, string | null>()
  const unresolved = new Map<string, string | null>()
  for (const e of timeline) {
    const r = resolveCausation(e.causationId, idx)
    resolved.set(e.id, r)
    if (e.causationId && !r) unresolved.set(e.id, e.causationId)
  }

  // id -> title for related-event labels.
  const titles = new Map<string, string>()
  for (const e of timeline) titles.set(e.id, e.summary ?? humanize(e.eventType))

  // id -> children (events that resolve their causation to this id).
  const childrenOf = new Map<string, string[]>()
  for (const e of timeline) {
    const parent = resolved.get(e.id)
    if (parent) {
      const arr = childrenOf.get(parent) ?? []
      arr.push(e.id)
      childrenOf.set(parent, arr)
    }
  }

  const msOf = (id: string): number => {
    const e = idx.byId.get(id)
    return e ? new Date(e.occurredAt).getTime() : 0
  }

  const events: TraceEvent[] = timeline.map((e) => {
    const kind = eventTypeToKind(e.eventType)
    const system = systemToSystemId(e.system, kind)
    const status = outcomeToStatus(e.outcome, e.eventType)
    const parent = resolved.get(e.id) ?? null

    const details: Record<string, string> = {}
    if (e.summary) details.Summary = e.summary
    if (e.nodeId) details.Node = e.nodeId
    // Real business-context and related-identity evidence (no fabrication): the
    // trace row carries these ids when the engine/dispatcher recorded them.
    if (e.dealId) details.Deal = e.dealId
    if (e.propertyId) details.Property = e.propertyId
    if (e.personId) details.Client = e.personId
    if (e.transactionDocumentId) details.Document = e.transactionDocumentId
    if (e.taskId) details.Task = e.taskId
    if (e.signatureRequestId) details.Signature = e.signatureRequestId
    if (e.workflowTransitionId) details.Transition = e.workflowTransitionId
    if (system === 'Unknown' && e.system) details['Raw System'] = e.system
    const rawCause = unresolved.get(e.id)
    if (rawCause) details['Cause Ref'] = rawCause
    if (e.metadata) {
      for (const [k, v] of Object.entries(e.metadata)) {
        if (Object.keys(details).length >= 10) break
        if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
          details[k] = String(v)
        } else if (v != null) {
          details[k] = JSON.stringify(v)
        }
      }
    }

    const relatedEventIds: RelatedEventRef[] = []
    if (parent) {
      relatedEventIds.push({
        id: parent,
        title: titles.get(parent) ?? parent,
        offsetMs: msOf(e.id) - msOf(parent),
      })
    }
    for (const childId of childrenOf.get(e.id) ?? []) {
      relatedEventIds.push({
        id: childId,
        title: titles.get(childId) ?? childId,
        offsetMs: msOf(childId) - msOf(e.id),
      })
    }
    relatedEventIds.sort((a, b) => a.offsetMs - b.offsetMs)

    const tags = [e.eventType.toLowerCase(), e.system.toLowerCase(), status.toLowerCase()].filter(
      Boolean,
    )

    return {
      id: e.id,
      correlationId,
      causationId: parent,
      commandId: e.commandId,
      domainEventId: e.domainEventId,
      workflowNodeId: e.nodeId ?? null,
      kind,
      type: e.eventType,
      title: e.summary ?? humanize(e.eventType),
      subtitle: KIND_TOKENS[kind].label,
      system,
      status,
      occurredAt: e.occurredAt,
      offsetMs: e.relativeMs,
      durationMs: e.durationMs ?? 0,
      details,
      payload: e.metadata,
      tags: [...new Set(tags)],
      relatedEventIds,
    }
  })

  const systems = new Set(events.map((ev) => ev.system))
  const root = events.find((ev) => ev.causationId === null) ?? events[0]

  let status: TraceSummary['status'] = 'Completed'
  if (events.some((ev) => ev.status === 'Failed')) status = 'Failed'
  else if (!timeline.some((e) => e.eventType === 'WORKFLOW_COMPLETED')) status = 'InProgress'

  const durationMs =
    inspection.expectedVsActual.workflowElapsedMs > 0
      ? inspection.expectedVsActual.workflowElapsedMs
      : Math.max(
          0,
          (inspection.endIso ? msOfIso(inspection.endIso) : 0) -
            (inspection.startIso ? msOfIso(inspection.startIso) : 0),
        )

  const summary: TraceSummary = {
    correlationId,
    rootTitle: root?.title ?? inspection.definitionKey ?? 'Workflow Execution',
    rootKind: root?.kind ?? 'Workflow',
    durationMs,
    eventCount: events.length,
    systemCount: systems.size,
    status,
    // Real business-context evidence carried through from the trace rows. Grok's
    // mockup invented these; we surface the ACTUAL deal/property/person ids the
    // engine recorded (first non-null across the timeline) and, when the read
    // side resolved them, the human-readable labels.
    businessContext: {
      dealId: resolvedBusinessContext?.dealId ?? firstOf(timeline, 'dealId'),
      deal: resolvedBusinessContext?.deal ?? firstOf(timeline, 'dealId'),
      property:
        resolvedBusinessContext?.property ??
        resolvedBusinessContext?.propertyId ??
        firstOf(timeline, 'propertyId'),
      client:
        resolvedBusinessContext?.client ??
        resolvedBusinessContext?.personId ??
        firstOf(timeline, 'personId'),
      workflow: inspection.definitionKey ?? undefined,
    },
  }

  return { summary, events }
}

/** First non-empty value of an optional TimelineEntry field across a timeline. */
function firstOf(
  timeline: TimelineEntry[],
  field: keyof Pick<
    TimelineEntry,
    'dealId' | 'propertyId' | 'personId' | 'transactionDocumentId'
  >,
): string | undefined {
  for (const e of timeline) {
    const v = e[field]
    if (v != null && v !== '') return String(v)
  }
  return undefined
}

/**
 * Reverse the adapter's TimelineEntry projection so console read-model events
 * can be fed back into the engine's pure causal projection (lib/causal-graph).
 *
 * The adapter preserves the ORIGINAL trace-event identity (`TraceEvent.id` is
 * the inspection timeline id) and `causationId` already points at a sibling
 * event id, so round-tripping keeps the DAG connected and grouped just like
 * the Runtime Inspector. `system` carries the console's SystemId vocabulary,
 * which only affects same-subsystem chain collapse granularity (not topology).
 */
export function toTimelineEntries(events: TraceEvent[]): TimelineEntry[] {
  return events.map((e) => ({
    id: e.id,
    occurredAt: e.occurredAt,
    relativeMs: e.offsetMs,
    eventType: e.type,
    system: e.system,
    summary: e.title,
    outcome: e.status,
    durationMs: e.durationMs,
    nodeId: null,
    causationId: e.causationId,
    commandId: e.commandId,
    domainEventId: e.domainEventId,
    metadata: (e.payload as Record<string, unknown> | null) ?? null,
  }))
}

/**
 * Adapt the canonical Flight Recorder transaction read model into the console
 * read-model. The console Trace Map renders the EXACT persisted master workflow
 * graph (with execution state), the Timeline renders only real durable events,
 * and event->node mapping comes from the read model's immutable ids.
 */
export function adaptFlightRecorderTransaction(
  tx: FlightRecorderTransaction,
): FlightRecorderTrace {
  const primary = tx.workflows[0] ?? null
  const baseMs = tx.events.length
    ? Math.min(
        ...tx.events
          .map((e) => new Date(e.occurredAt).getTime())
          .filter((t) => !Number.isNaN(t)),
      )
    : 0

  const events: TraceEvent[] = tx.events.map((e) => {
    const kind = eventTypeToKind(e.eventType)
    const system = systemToSystemId(e.sourceSystem, kind)
    const status = outcomeToStatus(e.outcome, e.eventType)
    const details: Record<string, string> = {}
    if (e.summary) details.Summary = e.summary
    if (e.workflowNodeId) {
      details.Node = e.mappedWorkflowNode?.name ?? e.workflowNodeId
      details['Node ID'] = e.workflowNodeId
    }
    if (e.commandId) details.Command = e.commandId
    if (e.domainEventId) details['Domain Event'] = e.domainEventId
    if (e.documentId) details.Document = e.documentId
    if (e.signatureRequestId) details.Signature = e.signatureRequestId
    if (system === 'Unknown' && e.sourceSystem) details['Raw System'] = e.sourceSystem
    if (e.metadata) {
      for (const [k, v] of Object.entries(e.metadata)) {
        if (Object.keys(details).length >= 10) break
        if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
          details[k] = String(v)
        } else if (v != null) {
          details[k] = JSON.stringify(v)
        }
      }
    }
    return {
      id: e.eventId,
      correlationId: e.correlationId ?? tx.transaction.correlationId ?? 'txn',
      causationId: e.causationId,
      commandId: e.commandId,
      domainEventId: e.domainEventId,
      workflowNodeId: e.workflowNodeId ?? null,
      kind,
      type: e.eventType,
      title: e.summary ?? humanize(e.eventType),
      subtitle: KIND_TOKENS[kind].label,
      system,
      status,
      occurredAt: e.occurredAt,
      offsetMs: Math.max(0, new Date(e.occurredAt).getTime() - baseMs),
      durationMs: e.durationMs ?? 0,
      details,
      payload: e.metadata,
      tags: [
        ...new Set([
          e.eventType.toLowerCase(),
          e.sourceSystem.toLowerCase(),
          status.toLowerCase(),
        ]),
      ].filter(Boolean),
      relatedEventIds: [],
    }
  })

  const root = events[0] ?? null
  const workflow = primary ? buildConsoleWorkflow(primary) : undefined

  const summary: TraceSummary = {
    correlationId: tx.transaction.correlationId ?? primary?.workflowInstanceId ?? 'txn',
    rootTitle: root?.title ?? primary?.definitionKey ?? 'Workflow Execution',
    rootKind: root?.kind ?? 'Workflow',
    durationMs: events.length ? Math.max(0, ...events.map((e) => e.offsetMs)) : 0,
    eventCount: events.length,
    systemCount: new Set(events.map((e) => e.system)).size,
    status: events.some((e) => e.status === 'Failed')
      ? 'Failed'
      : tx.transaction.status === 'active'
        ? 'InProgress'
        : 'Completed',
    businessContext: {
      dealId: tx.transaction.dealId ?? undefined,
      deal: tx.transaction.dealId ?? undefined,
      property: tx.transaction.property ?? undefined,
      client: tx.transaction.client ?? undefined,
      workflow: primary?.definitionKey ?? undefined,
    },
  }

  return { summary, events, workflow }
}

function buildConsoleWorkflow(wf: FlightRecorderWorkflow): ConsoleWorkflowView {
  const nodes: ConsoleWorkflowNode[] = Object.values(wf.graph.nodes).map((n) => {
    const st = wf.nodeStates[n.id]
    return {
      id: n.id,
      name: n.name ?? n.id,
      type: n.type ?? 'node',
      semanticKind: nodeTypeToKind(n.type ?? ''),
      state: st?.state ?? 'NOT_VISITED',
    }
  })
  const transitions: ConsoleWorkflowTransition[] = []
  for (const [fromId, n] of Object.entries(wf.graph.nodes)) {
    for (const t of n.transitions ?? []) transitions.push({ from: fromId, to: t.to, name: t.name })
  }
  return {
    workflowInstanceId: wf.workflowInstanceId,
    definitionKey: wf.definitionKey,
    definitionVersion: wf.definitionVersion,
    currentNodeId: wf.currentNodeId,
    nodes,
    transitions,
  }
}

function msOfIso(iso: string): number {
  const t = new Date(iso).getTime()
  return Number.isNaN(t) ? 0 : t
}

