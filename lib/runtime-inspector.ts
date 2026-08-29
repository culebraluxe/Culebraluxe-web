// ---------------------------------------------------------------------------
// RUNTIME-INSPECTOR — reconstruct runtime execution evidence over design-time
// topology.
//
// mapper(definition) renders the expected topology; mapper(definition,
// executionOverlay) adds actual runtime evidence. This module is the pure,
// testable reconstruction: given a ProcessGraph (from the Business Process
// Mapper / XML definition) and immutable TraceEvents, compute node/transition
// states, the chronological timeline, expected-vs-actual diagnostics, and the
// time-machine state at an arbitrary timestamp T.
//
// VISUAL REPLAY ONLY — it never re-executes business logic.
// ---------------------------------------------------------------------------

import type { ProcessGraph } from '../workflow_engine/lib/workflow/types'
import type { TraceEvent } from './workflow-trace'

export type NodeRuntimeState =
  | 'NOT_VISITED'
  | 'COMPLETED'
  | 'CURRENT'
  | 'FAILED'
  | 'RECOVERED'

export type NodeRuntime = {
  nodeId: string
  state: NodeRuntimeState
  executionCount: number
  enteredAt: string | null
  completedAt: string | null
  durationMs: number | null
  lastOutcome: string | null
  /** The event that caused this node to enter (causation). */
  triggerEventId: string | null
}

export type TransitionRuntime = {
  fromNodeId: string
  toNodeId: string
  traversedCount: number
  lastTakenAt: string | null
}

export type TimelineEntry = {
  id: string
  occurredAt: string
  relativeMs: number
  eventType: string
  system: string
  summary: string | null
  outcome: string | null
  durationMs: number | null
  nodeId: string | null
  causationId: string | null
  /** Stable identity projections used by the causal graph overlay. */
  commandId: string | null
  domainEventId: string | null
  metadata: Record<string, unknown> | null
  /**
   * Real business-context and related-identity evidence carried through from the
   * trace row so the Flight Recorder console can surface Grok-style "added
   * information" with REAL data (deal/property/person, task, signature, node,
   * transition). Nullable/optional: not every event belongs to every object and
   * the engine may not yet populate some of them.
   */
  dealId?: string | null
  propertyId?: string | null
  personId?: string | null
  transactionDocumentId?: string | null
  taskId?: string | null
  signatureRequestId?: string | null
  workflowDefinitionKey?: string | null
  workflowTransitionId?: string | null
}

export type ExpectedVsActual = {
  nodesExpected: number
  nodesVisited: number
  transitionsTaken: number
  unexpectedTransitions: number
  repeatedNodes: number
  repeatedTransitions: number
  failedEvents: number
  recoveredFailures: number
  currentNode: string | null
  workflowElapsedMs: number
}

export type RuntimeInspection = {
  workflowInstanceId: string
  definitionKey: string | null
  definitionVersion: number | null
  nodes: NodeRuntime[]
  transitions: TransitionRuntime[]
  timeline: TimelineEntry[]
  expectedVsActual: ExpectedVsActual
  startIso: string | null
  endIso: string | null
  currentNodeId: string | null
}

const NODE_ENTERED = 'NODE_ENTERED'
const NODE_COMPLETED = 'NODE_COMPLETED'
const TRANSITION_TAKEN = 'TRANSITION_TAKEN'
const WORKFLOW_COMPLETED = 'WORKFLOW_COMPLETED'

function isoToMs(iso: string): number {
  const t = new Date(iso).getTime()
  return Number.isNaN(t) ? 0 : t
}

/** Filter events that occurred at or before T (time-machine reconstruction). */
export function eventsAtOrBefore(events: TraceEvent[], tIso: string | null): TraceEvent[] {
  if (!tIso) return events
  const t = isoToMs(tIso)
  return events.filter((e) => isoToMs(e.occurredAt) <= t)
}

/** The current node id: the latest NODE_ENTERED not superseded by a later
 *  NODE_COMPLETED for the same node (and not a completed workflow). */
export function computeCurrentNode(events: TraceEvent[]): string | null {
  if (events.some((e) => e.eventType === WORKFLOW_COMPLETED)) return null
  const entered = events
    .filter((e) => e.eventType === NODE_ENTERED && e.workflowNodeId)
    .sort((a, b) => isoToMs(a.occurredAt) - isoToMs(b.occurredAt))
  for (let i = entered.length - 1; i >= 0; i--) {
    const nodeId = entered[i].workflowNodeId as string
    const enteredAt = isoToMs(entered[i].occurredAt)
    const completedAfter = events.some(
      (e) =>
        e.eventType === NODE_COMPLETED &&
        e.workflowNodeId === nodeId &&
        isoToMs(e.occurredAt) >= enteredAt,
    )
    if (!completedAfter) return nodeId
  }
  return null
}

/** Reconstruct per-node runtime state over a graph + trace events. */
export function buildNodeRuntimes(
  graph: ProcessGraph,
  events: TraceEvent[],
  currentNodeId: string | null,
): NodeRuntime[] {
  return Object.keys(graph.nodes).map((nodeId) => {
    const entered = events
      .filter((e) => e.eventType === NODE_ENTERED && e.workflowNodeId === nodeId)
      .sort((a, b) => isoToMs(a.occurredAt) - isoToMs(b.occurredAt))
    const completed = events.filter(
      (e) => e.eventType === NODE_COMPLETED && e.workflowNodeId === nodeId,
    )
    const failed = events.filter(
      (e) => e.eventType === 'FAILURE' && e.workflowNodeId === nodeId,
    )
    const recovered = events.filter(
      (e) => e.eventType === 'RECOVERED' && e.workflowNodeId === nodeId,
    )
    const workflowCompleted = events.some((e) => e.eventType === WORKFLOW_COMPLETED)

    const executionCount = entered.length
    let state: NodeRuntimeState = 'NOT_VISITED'
    if (executionCount > 0) {
      if (nodeId === currentNodeId) state = 'CURRENT'
      else if (completed.length > 0) state = 'COMPLETED'
      // A terminal node entered but never NODE_COMPLETED is done once the
      // whole workflow has completed (no lingering "current" node).
      else if (workflowCompleted) state = 'COMPLETED'
      else if (recovered.length > 0 && failed.length > 0) state = 'RECOVERED'
      else if (failed.length > 0) state = 'FAILED'
      else state = 'CURRENT'
    }

    const first = entered[0]
    const lastCompleted = completed[completed.length - 1]
    const durationMs =
      first && lastCompleted
        ? Math.max(0, isoToMs(lastCompleted.occurredAt) - isoToMs(first.occurredAt))
        : null

    return {
      nodeId,
      state,
      executionCount,
      enteredAt: first?.occurredAt ?? null,
      completedAt: lastCompleted?.occurredAt ?? null,
      durationMs,
      lastOutcome: lastCompleted?.outcome ?? null,
      triggerEventId: first?.causationId ?? null,
    }
  })
}

/** Traversed transitions from TRANSITION_TAKEN events. */
export function buildTransitionRuntimes(events: TraceEvent[]): TransitionRuntime[] {
  const map = new Map<string, TransitionRuntime>()
  for (const e of events) {
    if (e.eventType !== TRANSITION_TAKEN || !e.workflowNodeId) continue
    const from = e.workflowNodeId
    const to = e.workflowTransitionId ?? '?'
    const key = `${from}→${to}`
    const existing = map.get(key) ?? {
      fromNodeId: from,
      toNodeId: to,
      traversedCount: 0,
      lastTakenAt: null,
    }
    existing.traversedCount++
    if (!existing.lastTakenAt || e.occurredAt > existing.lastTakenAt) {
      existing.lastTakenAt = e.occurredAt
    }
    map.set(key, existing)
  }
  return [...map.values()].sort((a, b) =>
    (a.lastTakenAt ?? '').localeCompare(b.lastTakenAt ?? ''),
  )
}

/** Whether a transition (from→to) is a legal edge in the graph. */
function isLegalTransition(graph: ProcessGraph, from: string, to: string): boolean {
  const node = graph.nodes[from]
  if (!node || !node.transitions) return false
  return node.transitions.some((t) => t.to === to)
}

/** Chronological timeline with relative elapsed time from the earliest event. */
export function buildTimeline(events: TraceEvent[]): TimelineEntry[] {
  const sorted = [...events].sort((a, b) => isoToMs(a.occurredAt) - isoToMs(b.occurredAt))
  const start = sorted.length > 0 ? isoToMs(sorted[0].occurredAt) : 0
  return sorted.map((e, i) => ({
    id: e.sourceEventId ?? `evt-${i}`,
    occurredAt: e.occurredAt,
    relativeMs: Math.max(0, isoToMs(e.occurredAt) - start),
    eventType: e.eventType,
    system: e.system,
    summary: e.summary,
    outcome: e.outcome,
    durationMs: e.durationMs,
    nodeId: e.workflowNodeId,
    causationId: e.causationId,
    commandId: e.commandId,
    domainEventId: e.domainEventId,
    metadata: e.metadata,
    dealId: e.dealId ?? null,
    propertyId: e.propertyId ?? null,
    personId: e.personId ?? null,
    transactionDocumentId: e.transactionDocumentId ?? null,
    taskId: e.taskId ?? null,
    signatureRequestId: e.signatureRequestId ?? null,
    workflowDefinitionKey: e.workflowDefinitionKey ?? null,
    workflowTransitionId: e.workflowTransitionId ?? null,
  }))
}

/** Expected-vs-actual diagnostics. Unused legal branches are NOT anomalies. */
export function buildExpectedVsActual(
  graph: ProcessGraph,
  events: TraceEvent[],
  currentNodeId: string | null,
): ExpectedVsActual {
  const nodeRuntimes = buildNodeRuntimes(graph, events, currentNodeId)
  const transitions = buildTransitionRuntimes(events)

  const nodesVisited = nodeRuntimes.filter((n) => n.executionCount > 0).length
  const repeatedNodes = nodeRuntimes.filter((n) => n.executionCount > 1).length
  const unexpectedTransitions = transitions.filter(
    (t) => !isLegalTransition(graph, t.fromNodeId, t.toNodeId),
  ).length
  const repeatedTransitions = transitions.filter((t) => t.traversedCount > 1).length
  const failedEvents = events.filter((e) => e.eventType === 'FAILURE').length
  const recoveredFailures = events.filter((e) => e.eventType === 'RECOVERED').length

  const start = events.length > 0 ? isoToMs(events[0].occurredAt) : 0
  const end = events.reduce((max, e) => Math.max(max, isoToMs(e.occurredAt)), start)

  return {
    nodesExpected: Object.keys(graph.nodes).length,
    nodesVisited,
    transitionsTaken: transitions.length,
    unexpectedTransitions,
    repeatedNodes,
    repeatedTransitions,
    failedEvents,
    recoveredFailures,
    currentNode: currentNodeId,
    workflowElapsedMs: Math.max(0, end - start),
  }
}

/** Full Runtime Inspector payload for a workflow instance. */
export function buildRuntimeInspection(
  workflowInstanceId: string,
  graph: ProcessGraph,
  events: TraceEvent[],
  atTimestampIso: string | null = null,
): RuntimeInspection {
  const atEvents = eventsAtOrBefore(events, atTimestampIso)
  const currentNodeId = computeCurrentNode(atEvents)
  return {
    workflowInstanceId,
    definitionKey: events.find((e) => e.workflowDefinitionKey)?.workflowDefinitionKey ?? null,
    definitionVersion:
      events.find((e) => e.workflowDefinitionVersion != null)?.workflowDefinitionVersion ?? null,
    nodes: buildNodeRuntimes(graph, atEvents, currentNodeId),
    transitions: buildTransitionRuntimes(atEvents),
    timeline: buildTimeline(atEvents),
    expectedVsActual: buildExpectedVsActual(graph, atEvents, currentNodeId),
    startIso: atEvents.length > 0 ? atEvents[0].occurredAt : null,
    endIso: atEvents.length > 0 ? atEvents[atEvents.length - 1].occurredAt : null,
    currentNodeId,
  }
}


