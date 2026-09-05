// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-05 — Lead PRE dependency graph & fake-edge test.
//
// Decide SPLIT by asking "does Smith B require Smith A's OUTPUT?" — not "could
// several Smiths work on this?". Real dependency edges keep work sequential;
// only genuinely independent implementation nodes may run in parallel.
//
// Parallelism is an optimization. Correctness is an invariant. Ambiguous
// independence and any real dependency favor sequential execution.
//
// Pure + DB-free. Lead PRE still chooses SOLO | SMITH | SPLIT | HOLD; this only
// makes a SPLIT decision require explicit graph justification.
// ---------------------------------------------------------------------------

export type SmithWorkNode = {
  id: string
  purpose: string
  /** Symbolic inputs this node needs (matched against upstream outputs). */
  inputs: string[]
  /** Symbolic outputs this node produces. */
  outputs: string[]
  dependsOn: string[]
  /** Bounded mutation authority scope. */
  scope: string
}

export type LayerPlan = {
  layers: string[][]
  valid: boolean
  errors: string[]
}

/**
 * Topologically layer Smith nodes: each layer holds only nodes whose
 * dependencies are satisfied by earlier layers, and is capped at
 * concurrencyCap. Real edges force sequential layers; independent nodes share
 * a layer (parallelism only within the cap).
 */
export function planSmithLayers(input: {
  nodes: SmithWorkNode[]
  concurrencyCap?: number
}): LayerPlan {
  const cap = Math.max(1, input.concurrencyCap ?? 1)
  const byId = new Map(input.nodes.map((n) => [n.id, n]))
  const errors: string[] = []

  // Every dependency must reference a known node.
  for (const n of input.nodes) {
    for (const d of n.dependsOn) {
      if (!byId.has(d)) errors.push(`${n.id} depends on unknown node ${d}`)
    }
  }
  // Cycle detection (Kahn).
  const indeg = new Map<string, number>()
  for (const n of input.nodes) indeg.set(n.id, n.dependsOn.length)
  const adj = new Map<string, string[]>()
  for (const n of input.nodes) {
    for (const d of n.dependsOn) {
      const list = adj.get(d) ?? []
      list.push(n.id)
      adj.set(d, list)
    }
  }
  const queue: string[] = []
  for (const [id, deg] of indeg) if (deg === 0) queue.push(id)
  const order: string[] = []
  while (queue.length) {
    const cur = queue.shift()!
    order.push(cur)
    for (const next of adj.get(cur) ?? []) {
      const d = indeg.get(next)! - 1
      indeg.set(next, d)
      if (d === 0) queue.push(next)
    }
  }
  if (order.length !== input.nodes.length) {
    return {
      layers: [],
      valid: false,
      errors: errors.length > 0 ? errors : ['dependency cycle detected in Smith plan'],
    }
  }

  // Greedy layering over topo order: a node joins the earliest layer where it
  // is still valid (its deps are in strictly earlier layers) and layer < cap.
  const layers: string[][] = []
  const layerOf = new Map<string, number>()
  const inLayer = (node: SmithWorkNode, li: number): boolean => {
    const layer = layerOf.get(node.id)
    if (layer === undefined) return true
    return layer < li
  }
  const addToLayer = (id: string, li: number): void => {
    while (layers.length <= li) layers.push([])
    layers[li].push(id)
    layerOf.set(id, li)
  }
  // Place nodes in topological order so every dependency already has a layer.
  for (const id of order) {
    const n = byId.get(id)!
    if (n.dependsOn.length === 0) {
      addToLayer(id, 0)
      continue
    }
    const depLayer = Math.max(...n.dependsOn.map((d) => layerOf.get(d) ?? 0))
    addToLayer(id, depLayer + 1)
  }
  // Enforce the cap by splitting oversized layers into chunks.
  const bounded: string[][] = []
  for (const layer of layers) {
    for (let i = 0; i < layer.length; i += cap) bounded.push(layer.slice(i, i + cap))
  }
  // NOTE: nodes within a chunk share no dependency edge by construction, so the
  // bounded layers remain parallelism-safe (independence is proven).
  return { layers: bounded, valid: errors.length === 0, errors }
}

/**
 * SPLIT eligibility: true only when every proposed sibling is pairwise
 * independent (no node depends on another sibling's output). Ambiguous or
 * dependent sets are NOT split-eligible (conservative -> sequential).
 */
export function splitEligibility(nodes: SmithWorkNode[]): {
  eligible: boolean
  reason: string
} {
  if (nodes.length <= 1) return { eligible: false, reason: 'splitting requires more than one sibling' }
  const ids = new Set(nodes.map((n) => n.id))
  for (const n of nodes) {
    const internalDeps = n.dependsOn.filter((d) => ids.has(d))
    if (internalDeps.length > 0) {
      return {
        eligible: false,
        reason: `${n.id} depends on a sibling (${internalDeps.join(',')}) -> sequential, not SPLIT`,
      }
    }
  }
  return { eligible: true, reason: 'siblings are pairwise independent' }
}

/**
 * Fake-edge detector: an edge A -> B is REAL only when B's declared inputs
 * consume at least one of A's declared outputs. An edge with no such overlap is
 * a fake edge that unnecessarily serializes otherwise-independent work.
 */
export function fakeEdgeCandidates(nodes: SmithWorkNode[]): Array<{ from: string; to: string }> {
  const byId = new Map(nodes.map((n) => [n.id, n]))
  const fake: Array<{ from: string; to: string }> = []
  for (const b of nodes) {
    for (const dep of b.dependsOn) {
      const a = byId.get(dep)
      if (!a) continue
      const consumes = b.inputs.some((i) => a.outputs.includes(i))
      if (!consumes) fake.push({ from: a.id, to: b.id })
    }
  }
  return fake
}
