// ---------------------------------------------------------------------------
// PROCESS-PROFILER — classify a workflow execution timeline into machine /
// human / external wait segments (Business Process Profiler).
//
// Pure, deterministic, read-side only. Given the design-time node types and the
// chronological trace timeline, it segments the wall-clock elapsed time into
// alternating categories:
//   machine  — the system is actively processing (transitions, commands,
//              decisions, orchestrating; also any interval with no workflow
//              evidence yet, e.g. a command-only trace)
//   human    — the workflow is idle at a human-task node awaiting a person
//   external — the workflow is waiting on an external actor (timer/job fire,
//              provider/signature side-effect)
//
// A "wait" segment runs from a NODE_ENTERED at a wait node (task / timer) until
// the next event that leaves it (TRANSITION_TAKEN) or the process terminates.
// Everything else is attributed as machine time. The result is the "why did the
// workflow take this long" answer the Runtime Inspector surfaces.
// ---------------------------------------------------------------------------

import type { TimelineEntry } from './runtime-inspector'

export type WaitCategory = 'machine' | 'human' | 'external'

export type ProfileSegment = {
  category: WaitCategory
  fromIso: string
  toIso: string
  durationMs: number
  /** The workflow node being sat at during the segment ('' = in transit). */
  nodeId: string
}

export type WaitBreakdown = {
  count: number
  durationMs: number
  pct: number
}

export type NodeWait = {
  nodeId: string
  label: string
  category: WaitCategory
  durationMs: number
  enteredAt: string
}

export type ProcessProfile = {
  totalMs: number
  segments: ProfileSegment[]
  breakdown: Record<WaitCategory, WaitBreakdown>
  nodeWaits: NodeWait[]
  hasWorkflowEvidence: boolean
}

const CATEGORIES: WaitCategory[] = ['machine', 'human', 'external']

function ms(iso: string): number {
  const t = new Date(iso).getTime()
  return Number.isNaN(t) ? 0 : t
}

/** The wait category a node type implies (default: machine/active). */
function categoryFor(nodeType: string | undefined): WaitCategory {
  if (nodeType === 'task') return 'human'
  if (nodeType === 'timer') return 'external'
  return 'machine'
}

function emptyBreakdown(): Record<WaitCategory, WaitBreakdown> {
  return {
    machine: { count: 0, durationMs: 0, pct: 0 },
    human: { count: 0, durationMs: 0, pct: 0 },
    external: { count: 0, durationMs: 0, pct: 0 },
  }
}

export function emptyProfile(): ProcessProfile {
  return {
    totalMs: 0,
    segments: [],
    breakdown: emptyBreakdown(),
    nodeWaits: [],
    hasWorkflowEvidence: false,
  }
}

/**
 * Segment the timeline into machine / human / external waits. `nodeTypes` maps
 * workflow node id -> design-time node type (task / timer / command / ...).
 * Deterministic: identical inputs always produce identical segments.
 */
export function profileProcess(
  nodeTypes: Record<string, string>,
  timeline: TimelineEntry[],
): ProcessProfile {
  const sorted = [...timeline].sort((a, b) => ms(a.occurredAt) - ms(b.occurredAt))
  if (sorted.length === 0) return emptyProfile()

  const nodeType = (id: string | null): string | undefined =>
    id ? nodeTypes[id] : undefined

  const segments: ProfileSegment[] = []
  const nodeDurs = new Map<string, NodeWait>()
  const nodeEntered = new Map<string, string>()

  let current: string | null = null
  let prevIso: string | null = null
  let total = 0
  let hasWorkflowEvidence = false

  for (const e of sorted) {
    // Attribute the interval (prevIso -> e.occurredAt) to the node we were
    // sitting at DURING that interval (the state before this event applies).
    if (prevIso !== null) {
      const dur = ms(e.occurredAt) - ms(prevIso)
      if (dur > 0) {
        const cat = current ? categoryFor(nodeType(current)) : 'machine'
        const nodeId = current ?? ''
        segments.push({ category: cat, fromIso: prevIso, toIso: e.occurredAt, durationMs: dur, nodeId })
        total += dur
        if (cat !== 'machine' && current) {
          const existing = nodeDurs.get(current) ?? {
            nodeId: current,
            label: nodeIdLabel(nodeTypes, current),
            category: cat,
            durationMs: 0,
            enteredAt: nodeEntered.get(current) ?? e.occurredAt,
          }
          existing.durationMs += dur
          nodeDurs.set(current, existing)
        }
      }
    }

    // Apply this event's effect on the "current node" state.
    if (e.eventType === 'NODE_ENTERED') {
      hasWorkflowEvidence = true
      current = e.nodeId
      nodeEntered.set(e.nodeId ?? '', e.occurredAt)
    } else if (e.eventType === 'TRANSITION_TAKEN') {
      hasWorkflowEvidence = true
      current = null // leaving the node we were sitting at
    } else if (e.eventType === 'WORKFLOW_COMPLETED' || e.eventType === 'WORKFLOW_FAILED') {
      current = null
    }
    prevIso = e.occurredAt
  }

  const breakdown = emptyBreakdown()
  for (const s of segments) {
    breakdown[s.category].count += 1
    breakdown[s.category].durationMs += s.durationMs
  }
  for (const cat of CATEGORIES) {
    breakdown[cat].pct = total > 0 ? Math.round((breakdown[cat].durationMs / total) * 1000) / 10 : 0
  }

  const nodeWaits = [...nodeDurs.values()].sort((a, b) => b.durationMs - a.durationMs)

  return { totalMs: total, segments, breakdown, nodeWaits, hasWorkflowEvidence }
}

function nodeIdLabel(nodeTypes: Record<string, string>, id: string): string {
  const type = nodeTypes[id]
  return type ? `${id} (${type})` : id
}
