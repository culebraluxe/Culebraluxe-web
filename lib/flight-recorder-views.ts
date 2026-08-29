import type { TraceEvent, SystemId } from './flight-recorder-adapter'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-VIEWS — small PURE view-model projections shared by the
// Causality, System Swimlane, and Raw Events lenses. They consume the same
// TraceEvent[] presentation objects; they never touch the DB or add a model.
// ---------------------------------------------------------------------------

export type CausalEventPair = { from: TraceEvent; to: TraceEvent }

/**
 * Proven causal event pairs: `to.causationId` resolves to a loaded event `from`.
 * Chronology alone NEVER creates an edge — only explicit durable causation.
 */
export function buildCausalEventPairs(events: TraceEvent[]): CausalEventPair[] {
  const byId = new Map(events.map((e) => [e.id, e]))
  const pairs: CausalEventPair[] = []
  const seen = new Set<string>()
  for (const e of events) {
    if (!e.causationId) continue
    const cause = byId.get(e.causationId)
    if (!cause || cause.id === e.id) continue
    const key = `${cause.id}->${e.id}`
    if (seen.has(key)) continue
    seen.add(key)
    pairs.push({ from: cause, to: e })
  }
  return pairs
}

/** Causation references that point OUTSIDE the loaded event set (unresolved). */
export type UnresolvedCause = { event: TraceEvent; causeId: string }

export function buildUnresolvedCauses(events: TraceEvent[]): UnresolvedCause[] {
  const ids = new Set(events.map((e) => e.id))
  const out: UnresolvedCause[] = []
  for (const e of events) {
    if (e.causationId && !ids.has(e.causationId)) out.push({ event: e, causeId: e.causationId })
  }
  return out
}

/** Parent (cause) and child (effect) event ids of a selected event. */
export function buildSelectionCausality(
  events: TraceEvent[],
  selectedId: string,
): { parents: string[]; children: string[] } {
  const pairs = buildCausalEventPairs(events)
  const parents = new Set<string>()
  const children = new Set<string>()
  for (const p of pairs) {
    if (p.to.id === selectedId) parents.add(p.from.id)
    if (p.from.id === selectedId) children.add(p.to.id)
  }
  return { parents: [...parents], children: [...children] }
}

/** Preferred stable lane order; actual systems govern, Unknown always last. */
const LANE_PREFERENCE: SystemId[] = [
  'API Gateway',
  'Domain Model',
  'Workflow Engine',
  'Task Service',
  'BoldSign',
  'PostgreSQL',
]

/**
 * Group events by truthful normalized SystemId. Lanes follow a stable preferred
 * order when present, then any other systems, then Unknown (raw producers stay
 * visible via each event's raw system). An event appears exactly once.
 */
export function groupEventsBySystem(
  events: TraceEvent[],
): { system: SystemId; events: TraceEvent[] }[] {
  const lanes = new Map<SystemId, TraceEvent[]>()
  for (const e of events) {
    const arr = lanes.get(e.system) ?? []
    arr.push(e)
    lanes.set(e.system, arr)
  }
  const ordered: SystemId[] = [
    ...LANE_PREFERENCE.filter((s) => lanes.has(s)),
    ...[...lanes.keys()].filter((s) => !LANE_PREFERENCE.includes(s) && s !== 'Unknown'),
    ...(lanes.has('Unknown') ? (['Unknown'] as SystemId[]) : []),
  ]
  return ordered
    .map((system) => ({ system, events: lanes.get(system) ?? [] }))
    .filter((l) => l.events.length > 0)
}

/**
 * Raw Events projection: the ordered immutable fields worth exposing on a dense
 * evidence row (raw + normalized pairs preserved). Returns a stable label->value
 * list for one event.
 */
export type RawField = { key: string; value: string | null; mono?: boolean }

export function rawEventFields(e: TraceEvent): RawField[] {
  const pairs: RawField[] = [
    { key: 'eventType', value: e.type, mono: true },
    { key: 'eventId', value: e.id, mono: true },
    // RAW SYSTEM is the preserved producer value; NORMALIZED SYSTEM is the
    // Flight Recorder presentation classification. Never conflate them.
    { key: 'rawSystem', value: e.details['Raw System'] ?? e.system, mono: true },
    { key: 'normalizedSystem', value: e.system, mono: true },
    { key: 'status', value: e.status },
    { key: 'occurredAt', value: e.occurredAt, mono: true },
    { key: 'offsetMs', value: String(e.offsetMs), mono: true },
    { key: 'workflowNodeId', value: e.workflowNodeId ?? null, mono: true },
    { key: 'workflowInstanceId', value: e.details['Workflow Instance'] ?? null, mono: true },
    { key: 'correlationId', value: e.correlationId, mono: true },
    { key: 'causationId', value: e.causationId, mono: true },
    { key: 'commandId', value: e.commandId, mono: true },
    { key: 'domainEventId', value: e.domainEventId, mono: true },
    { key: 'documentId', value: e.details['Document'] ?? null, mono: true },
    { key: 'signatureRequestId', value: e.details['Signature'] ?? null, mono: true },
    { key: 'qaSimulation', value: (e.payload as { qa_simulation?: boolean } | null)?.qa_simulation === true ? 'true' : null, mono: true },
  ]
  return pairs.filter((p) => p.value != null && p.value !== '')
}

/**
 * An edge is causally "active" for selection ONLY when the selected causal node
 * is one of its endpoints. This deliberately excludes sibling edges (e.g.
 * P -> X when S is selected and P -> S, P -> X) so emphasis never implies a
 * relationship that is merely "nearby" rather than direct.
 */
export function isSelectedCausalEdge(
  edge: { source: string; target: string },
  selectedNodeId: string | null,
): boolean {
  if (!selectedNodeId) return false
  return edge.source === selectedNodeId || edge.target === selectedNodeId
}
