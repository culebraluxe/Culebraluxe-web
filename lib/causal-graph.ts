// ---------------------------------------------------------------------------
// CAUSAL-GRAPH — a read-only causal DAG projection over Flight Recorder trace
// events, laid out for the Runtime Inspector's graph overlay.
//
// This is a projection problem, not a new architecture: each trace event already
// carries a stable identity (`commandId` / `domainEventId` / `id`) and a
// `causationId` that points at the event which caused it. That chain is the DAG.
//
//   DomainEvent.causationId = the command that produced it (commandId)
//   DomainEvent.eventId     -> future event.causationId (event chains)
//   Command.causationId     = a parent command (cross-command causation)
//
// The module is pure and deterministic (important for the time machine and for
// tests): given the same events it always produces the same node/edge set and
// the same layered coordinates. It groups a command's stage events (received /
// completed / failed) into ONE command node and collapses linear same-subsystem
// chains so the graph stays a readable causal skeleton instead of a bowl of
// spaghetti. Rendering is left to the SVG component.
// ---------------------------------------------------------------------------

import type { TimelineEntry } from './runtime-inspector'

export type GraphColor =
  | 'command' // purple
  | 'domain' // blue
  | 'workflow' // green
  | 'task' // gold
  | 'external' // pink
  | 'persistence' // cyan
  | 'failure' // red
  | 'neutral'

export type CausalNode = {
  id: string
  label: string
  color: GraphColor
  system: string
  eventType: string
  /** 1, or >1 when this node aggregates several events (command stages, chains). */
  count: number
  members: TimelineEntry[]
  summary: string | null
  outcome: string | null
}

export type CausalEdge = {
  source: string
  target: string
}

export type CausalGraph = {
  nodes: CausalNode[]
  edges: CausalEdge[]
}

export type LayoutNode = CausalNode & {
  layer: number
  row: number
  x: number
  y: number
}

export type CausalLayout = {
  nodes: LayoutNode[]
  edges: CausalEdge[]
  width: number
  height: number
  nodeRadius: number
}

const NODE_R = 10
const LAYER_W = 220
const ROW_H = 66
const PAD = 36

/** Subsystem color classification from a trace event type. Failure takes priority. */
export function classifyEvent(eventType: string): GraphColor {
  const t = eventType.toUpperCase()
  if (t === 'FAILURE' || t === 'RETRY' || t === 'RECOVERED' || t.endsWith('_FAILED')) return 'failure'
  if (t.startsWith('COMMAND_')) return 'command'
  if (t.startsWith('DOMAIN_EVENT')) return 'domain'
  if (t.startsWith('WORKFLOW_') || t.startsWith('NODE_') || t.startsWith('TRANSITION_')) return 'workflow'
  if (t.startsWith('TASK_') || t.startsWith('TIMER_') || t.startsWith('JOB_')) return 'task'
  if (t.startsWith('SIGNATURE_')) return 'external'
  if (t.startsWith('DOCUMENT_')) return 'persistence'
  return 'neutral'
}

/** Canonical graph-node identity for a trace event (its OWN id, not its cause). */
function nodeKeyFor(e: TimelineEntry): string {
  if (e.eventType === 'DOMAIN_EVENT_EMITTED') {
    // A domain row carries both its own domainEventId and the causing command's
    // id (in commandId). Key on its OWN identity so it is not fused into the
    // command node.
    return e.domainEventId ? `dom:${e.domainEventId}` : `evt:${e.id}`
  }
  if (e.commandId) return `cmd:${e.commandId}`
  if (e.domainEventId) return `dom:${e.domainEventId}`
  return `evt:${e.id}`
}

/** Human-friendly short label (command type / domain event type). */
function labelFor(e: TimelineEntry): string {
  if (e.eventType === 'DOMAIN_EVENT_EMITTED') {
    const m = e.summary?.match(/^Domain event\s+(.+)$/)
    return m ? m[1] : e.eventType
  }
  if (e.commandId) {
    const m = e.summary?.match(/^Command\s+(.+?)\s+\w+$/)
    return m ? m[1] : e.eventType
  }
  return e.eventType
}

/**
 * Build the causal DAG from trace events. Command stage events are grouped into
 * one command node; edges follow `causationId`. Returns nodes + edges only.
 */
export function buildCausalGraph(events: TimelineEntry[]): CausalGraph {
  const groups = new Map<string, TimelineEntry[]>()
  for (const e of events) {
    const k = nodeKeyFor(e)
    const arr = groups.get(k) ?? []
    arr.push(e)
    groups.set(k, arr)
  }

  const nodes: CausalNode[] = []
  const causalKeyToNode = new Map<string, string>()
  for (const [id, members] of groups) {
    const first = members[0]
    const last = members[members.length - 1]
    const failed = members.some(
      (m) => m.outcome === 'FAILURE' || classifyEvent(m.eventType) === 'failure',
    )
    nodes.push({
      id,
      label: labelFor(first),
      color: failed ? 'failure' : classifyEvent(first.eventType),
      system: first.system,
      eventType: first.eventType,
      count: members.length,
      members,
      summary: last.summary ?? first.summary ?? null,
      outcome: last.outcome ?? first.outcome ?? null,
    })
    // Register ONLY the node's OWN identity as a resolvable causal key. A domain
    // row carries the causing command's id in `commandId`, which must never be
    // claimed by the domain node (that would steal the command->domain edge).
    const ownIsDomain = first.eventType === 'DOMAIN_EVENT_EMITTED'
    for (const m of members) {
      if (ownIsDomain) {
        if (m.domainEventId && !causalKeyToNode.has(m.domainEventId)) causalKeyToNode.set(m.domainEventId, id)
      } else {
        if (m.commandId && !causalKeyToNode.has(m.commandId)) causalKeyToNode.set(m.commandId, id)
        if (m.domainEventId && !causalKeyToNode.has(m.domainEventId)) causalKeyToNode.set(m.domainEventId, id)
      }
      if (!causalKeyToNode.has(m.id)) causalKeyToNode.set(m.id, id)
    }
  }

  // Edges flow CAUSE -> EFFECT (the earlier event caused the later one), so the
  // DAG reads left to right in time: source = the node referenced by a
  // causationId, target = the event that was caused by it.
  const edges: CausalEdge[] = []
  const seen = new Set<string>()
  for (const e of events) {
    if (!e.causationId) continue
    const effect = nodeKeyFor(e)
    const cause = causalKeyToNode.get(e.causationId)
    if (!cause || cause === effect) continue
    const key = `${cause}->${effect}`
    if (seen.has(key)) continue
    seen.add(key)
    edges.push({ source: cause, target: effect })
  }

  return collapseLinearChains(nodes, edges)
}

/**
 * Collapse maximal linear chains of consecutive same-subsystem nodes into a
 * single node, so e.g. five sequential persistence writes become one
 * "Persistence (5)" node. Strict: a node is only fused when it has exactly one
 * in and one out edge and shares color+system, so joins/splits are never fused
 * and edges never have to be re-targeted across a branch.
 */
export function collapseLinearChains(nodes: CausalNode[], edges: CausalEdge[]): CausalGraph {
  const nodeById = new Map(nodes.map((n) => [n.id, n]))
  const inMap = new Map<string, string[]>()
  const outMap = new Map<string, string[]>()
  for (const e of edges) {
    const a = outMap.get(e.source) ?? []
    a.push(e.target)
    outMap.set(e.source, a)
    const b = inMap.get(e.target) ?? []
    b.push(e.source)
    inMap.set(e.target, b)
  }
  const sameGrp = (a: string, b: string) => {
    const na = nodeById.get(a)!
    const nb = nodeById.get(b)!
    return na.color === nb.color && na.system === nb.system
  }

  const assigned = new Set<string>()
  const chains: string[][] = []
  for (const n of nodes) {
    if (assigned.has(n.id)) continue
    const outs = outMap.get(n.id) ?? []
    if (outs.length !== 1) continue // only linear heads start a chain
    const chain = [n.id]
    assigned.add(n.id)
    let cur = outs[0]
    while (!assigned.has(cur)) {
      if (!sameGrp(chain[chain.length - 1], cur)) break
      chain.push(cur)
      assigned.add(cur)
      const ins = inMap.get(cur) ?? []
      const couts = outMap.get(cur) ?? []
      if (ins.length !== 1 || couts.length !== 1) break // tail (join/split) stops here
      cur = couts[0]
    }
    if (chain.length >= 2) chains.push(chain)
  }
  if (chains.length === 0) return { nodes, edges }

  const groupOf = new Map<string, string>()
  for (const ch of chains) for (const id of ch) groupOf.set(id, ch[0])

  const membersOf = new Map<string, TimelineEntry[]>()
  for (const n of nodes) {
    const g = groupOf.get(n.id) ?? n.id
    const arr = membersOf.get(g) ?? []
    for (const m of n.members) arr.push(m)
    membersOf.set(g, arr)
  }

  const finalNodes: CausalNode[] = []
  for (const n of nodes) {
    const g = groupOf.get(n.id) ?? n.id
    if (g !== n.id) continue // fused into the head; not emitted separately
    const members = membersOf.get(g)!
    const last = members[members.length - 1]
    finalNodes.push({
      ...n,
      count: members.length,
      members,
      summary: last.summary ?? n.summary,
      outcome: last.outcome ?? n.outcome,
    })
  }

  const finalEdges: CausalEdge[] = []
  const seen = new Set<string>()
  for (const e of edges) {
    const s = groupOf.get(e.source) ?? e.source
    const t = groupOf.get(e.target) ?? e.target
    if (s === t) continue // internal chain edge
    const key = `${s}->${t}`
    if (seen.has(key)) continue
    seen.add(key)
    finalEdges.push({ source: s, target: t })
  }

  return { nodes: finalNodes, edges: finalEdges }
}

/**
 * Longest-path layered layout: roots on the left, a node one layer to the right
 * of its farthest predecessor. Within a layer, nodes are ordered by their
 * earliest member event (chronology) and spaced into even rows. Deterministic.
 */
export function layoutGraph(graph: CausalGraph): CausalLayout {
  const inMap = new Map<string, string[]>()
  const outMap = new Map<string, string[]>()
  for (const e of graph.edges) {
    const a = outMap.get(e.source) ?? []
    a.push(e.target)
    outMap.set(e.source, a)
    const b = inMap.get(e.target) ?? []
    b.push(e.source)
    inMap.set(e.target, b)
  }

  const memo = new Map<string, number>()
  const visiting = new Set<string>()
  const layerOf = (id: string): number => {
    const m = memo.get(id)
    if (m !== undefined) return m
    if (visiting.has(id)) return 0 // cycle guard: never loop forever
    visiting.add(id)
    const parents = inMap.get(id) ?? []
    let l = 0
    for (const p of parents) l = Math.max(l, layerOf(p) + 1)
    visiting.delete(id)
    memo.set(id, l)
    return l
  }

  if (graph.nodes.length === 0) {
    return { nodes: [], edges: [], width: PAD * 2, height: PAD * 2, nodeRadius: NODE_R }
  }

  const earliest = new Map<string, string>()
  for (const n of graph.nodes) {
    let min = n.members[0]?.occurredAt ?? ''
    for (const m of n.members) if (m.occurredAt < min) min = m.occurredAt
    earliest.set(n.id, min)
  }

  const layerNodes = new Map<number, LayoutNode[]>()
  for (const n of graph.nodes) {
    const layer = layerOf(n.id)
    const arr = layerNodes.get(layer) ?? []
    arr.push({ ...n, layer, row: 0, x: 0, y: 0 })
    layerNodes.set(layer, arr)
  }

  const layers = [...layerNodes.keys()].sort((a, b) => a - b)
  let maxRow = 0
  const laid: LayoutNode[] = []
  for (const layer of layers) {
    const arr = layerNodes
      .get(layer)!
      .sort((a, b) => (earliest.get(a.id)! < earliest.get(b.id)! ? -1 : earliest.get(a.id)! > earliest.get(b.id)! ? 1 : 0))
    arr.forEach((n, i) => {
      n.row = i
      n.x = PAD + layer * LAYER_W
      n.y = PAD + i * ROW_H + ROW_H / 2
    })
    laid.push(...arr)
    maxRow = Math.max(maxRow, arr.length)
  }

  const maxLayer = Math.max(...layers, 0)
  return {
    nodes: laid,
    edges: graph.edges,
    width: PAD * 2 + maxLayer * LAYER_W,
    height: PAD * 2 + Math.max(1, maxRow) * ROW_H,
    nodeRadius: NODE_R,
  }
}

