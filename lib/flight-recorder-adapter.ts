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

import type { RuntimeInspection, TimelineEntry } from './runtime-inspector'

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

export type SystemId =
  | 'API Gateway'
  | 'Domain Model'
  | 'Workflow Engine'
  | 'Task Service'
  | 'BoldSign'
  | 'PostgreSQL'

export type EventStatus = 'Success' | 'Failed' | 'Pending' | 'Skipped'

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
}

export const SYSTEM_CHIP: Record<SystemId, string> = {
  'API Gateway': 'bg-indigo-500/20 text-indigo-200 ring-1 ring-indigo-400/30',
  'Domain Model': 'bg-blue-500/20 text-blue-200 ring-1 ring-blue-400/30',
  'Workflow Engine': 'bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30',
  'Task Service': 'bg-amber-600/20 text-amber-200 ring-1 ring-amber-400/30',
  BoldSign: 'bg-pink-500/20 text-pink-200 ring-1 ring-pink-400/30',
  PostgreSQL: 'bg-cyan-500/20 text-cyan-200 ring-1 ring-cyan-400/30',
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
  // Bare operational signals (FAILURE / RETRY / RECOVERED) carry no subsystem;
  // surface them as Task and let the status chip carry the failure meaning.
  return 'Workflow'
}

/**
 * Map the engine's open-ended `system` string onto the console's closed
 * SystemId enum. The engine currently records 'command' | 'domain' |
 * 'workflow'; the fallbacks keep unknown producers stable and honest.
 */
export function systemToSystemId(system: string, kind: EventKind): SystemId {
  const s = system.toLowerCase()
  if (s.includes('domain')) return 'Domain Model'
  if (s.includes('command') || s.includes('api') || s.includes('gateway')) return 'API Gateway'
  if (s.includes('workflow')) return 'Workflow Engine'
  if (s.includes('task')) return 'Task Service'
  if (s.includes('signature') || s.includes('bolder') || s.includes('sig')) return 'BoldSign'
  if (s.includes('postgres') || s.includes('db') || s.includes('persist') || s.includes('sql'))
    return 'PostgreSQL'
  if (kind === 'Command') return 'API Gateway'
  if (kind === 'DomainEvent') return 'Domain Model'
  if (kind === 'Integration') return 'BoldSign'
  if (kind === 'Persistence') return 'PostgreSQL'
  return 'Workflow Engine'
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
  return 'Success'
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

function msOfIso(iso: string): number {
  const t = new Date(iso).getTime()
  return Number.isNaN(t) ? 0 : t
}

