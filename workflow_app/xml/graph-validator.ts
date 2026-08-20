// ---------------------------------------------------------------------------
// Generic ProcessGraph validator (Story 115).
//
// Validation operates on the runtime representation (`ProcessGraph`), NOT on
// XML, so future authoring formats (a visual editor, JSON, YAML) reuse the same
// validator. The engine is the final authority; this validator only rejects
// structures the engine cannot actually run, plus a small set of unambiguous
// authoring errors. It deliberately does NOT invent stylistic constraints.
//
// The engine handles cycles (blocker loops are intentional), so cycles are
// allowed and are not reported as errors.
// ---------------------------------------------------------------------------

import type {
  NodeDefinition,
  ProcessGraph,
  ProcessOutcome,
} from '../../workflow_engine/lib/workflow/types'

export interface GraphValidationResult {
  valid: boolean
  errors: string[]
  warnings: string[]
}

const VALID_OUTCOMES: ReadonlySet<string> = new Set<ProcessOutcome>([
  'completed',
  'cancelled',
  'failed',
  'conflict',
])

export function validateProcessGraph(graph: ProcessGraph): GraphValidationResult {
  const errors: string[] = []
  const warnings: string[] = []

  if (!graph || typeof graph !== 'object') {
    return { valid: false, errors: ['ProcessGraph must be an object'], warnings: [] }
  }

  const nodes = graph.nodes ?? {}
  const nodeIds = Object.keys(nodes)

  if (nodeIds.length === 0) {
    errors.push('ProcessGraph has no nodes')
  }

  if (!graph.startNodeId) {
    errors.push('ProcessGraph has no startNodeId')
  }

  const startNodes = nodeIds.filter((id) => nodes[id].type === 'start')
  if (startNodes.length === 0) {
    errors.push('ProcessGraph has no node of type "start"')
  } else if (startNodes.length > 1) {
    errors.push(`ProcessGraph has ${startNodes.length} start nodes; exactly one is required`)
  }

  if (graph.startNodeId && nodes[graph.startNodeId] === undefined) {
    errors.push(`startNodeId '${graph.startNodeId}' does not reference an existing node`)
  } else if (graph.startNodeId && nodes[graph.startNodeId]?.type !== 'start') {
    errors.push(`startNodeId '${graph.startNodeId}' does not reference a node of type "start"`)
  }

  for (const id of nodeIds) {
    validateNode(nodes[id], nodes, errors, warnings)
  }

  for (const ref of graph.displayOrder ?? []) {
    if (nodes[ref] === undefined) {
      warnings.push(`displayOrder references missing node '${ref}'`)
    }
  }

  return { valid: errors.length === 0, errors, warnings }
}

function validateNode(
  node: NodeDefinition,
  nodes: Record<string, NodeDefinition>,
  errors: string[],
  warnings: string[],
): void {
  if (!node.id) {
    errors.push('A node is missing its id')
    return
  }
  const id = node.id

  if (node.type === 'start') {
    if (!node.transitions || node.transitions.length === 0) {
      warnings.push(`start node '${id}' has no outgoing transition; the process would complete immediately`)
    }
  }

  if (node.type === 'end') {
    const outcome = node.outcome ?? 'completed'
    if (!VALID_OUTCOMES.has(outcome)) {
      errors.push(`end node '${id}' has invalid outcome '${outcome}'`)
    }
    if (node.transitions && node.transitions.length > 0) {
      warnings.push(`end node '${id}' declares transitions, which the engine ignores`)
    }
    return
  }

  if (node.type === 'command') {
    if (!node.commandType) {
      errors.push(`command node '${id}' has no commandType`)
    }
    if (!node.transitions || node.transitions.length === 0) {
      errors.push(`command node '${id}' has no success transition`)
    }
  }

  if (node.type === 'decision') {
    if (!node.transitions || node.transitions.length === 0) {
      errors.push(`decision node '${id}' has no transitions; the engine cannot route it`)
    }
    const transitionNames = new Set((node.transitions ?? []).map((t) => t.name))
    for (const d of node.decisions ?? []) {
      if (!d.condition || !d.condition.trim()) {
        errors.push(`decision node '${id}' has an <on> rule with an empty condition`)
      }
      if (!transitionNames.has(d.transition)) {
        errors.push(
          `decision node '${id}' <on> rule references transition '${d.transition}' which is not declared on the node`,
        )
      }
    }
  }

  if (node.type === 'fork') {
    if (!node.transitions || node.transitions.length === 0) {
      errors.push(`fork node '${id}' has no branches`)
    }
  }

  if (node.type === 'join') {
    if (!node.transitions || node.transitions.length === 0) {
      warnings.push(`join node '${id}' has no outgoing transition; the process may end here`)
    }
  }

  if (node.type === 'timer') {
    const timer = node.timer ?? {}
    if (!timer.dueAt && !timer.dueAtVariable) {
      errors.push(`timer node '${id}' must declare due-at or due-at-variable`)
    }
    if (timer.dueAt && Number.isNaN(new Date(timer.dueAt).getTime())) {
      errors.push(`timer node '${id}' has an invalid due-at date '${timer.dueAt}'`)
    }
    if (!node.transitions || node.transitions.length === 0) {
      errors.push(`timer node '${id}' has no resume transition; firing would fail`)
    }
  }

  const seenNames = new Set<string>()
  for (const t of node.transitions ?? []) {
    if (!t.name || !t.name.trim()) {
      errors.push(`node '${id}' has a transition with an empty name`)
    }
    if (seenNames.has(t.name)) {
      errors.push(`node '${id}' declares duplicate transition name '${t.name}'`)
    }
    seenNames.add(t.name)

    if (!t.to) {
      errors.push(`transition '${t.name}' on node '${id}' has no target (to)`)
    } else if (nodes[t.to] === undefined) {
      errors.push(`transition '${t.name}' on node '${id}' targets missing node '${t.to}'`)
    }

    if (t.required !== undefined && typeof t.required !== 'boolean') {
      errors.push(`transition '${t.name}' on node '${id}' has a non-boolean required flag`)
    }
  }

  if (node.priority !== undefined && (typeof node.priority !== 'number' || node.priority < 0)) {
    errors.push(`node '${id}' has an invalid priority '${node.priority}'`)
  }
}
