// ---------------------------------------------------------------------------
// Generic ProcessGraph validator (Story 115, hardened by ENG-14).
//
// This is Layer 3 of the four explicit workflow-definition validation layers
// (ENG-14 — Workflow Definition Validation / Static Analysis):
//
//   Layer 1  XML well-formedness       mini-xml.parseXml        (XmlParseError)
//   Layer 2  Engine grammar            xml-parser               (XmlGrammarError)
//   Layer 3  Generic graph semantics   this module              (GraphValidationResult)
//   Layer 4  Application contract      workflow_app/definitions/application-contract.ts
//
// Validation operates on the runtime representation (`ProcessGraph`), NOT on
// XML, so future authoring formats (a visual editor, JSON, YAML) reuse the same
// validator. The engine is the final authority; this validator only rejects
// structures the engine cannot actually run, plus a small set of unambiguous
// authoring errors. It deliberately does NOT invent stylistic constraints.
//
// ENG-14 additions (all deterministic, deploy-time, engine-grounded):
//   - unreachable-node reporting: any node with no path of transitions from
//     the start node is an error (dead weight; usually a typo or leftover).
//   - unsupported-node diagnostics: node types the engine has no handler for
//     are rejected instead of silently degrading to passthrough behavior.
//   - impossible-join / fork-join analysis, but ONLY where safely determinable:
//       * ERROR — a required fork branch that is a closed loop with no exit
//         (no end/leaf/fork/join reachable): its token can never complete, so
//         the process would hang and any join it feeds can never release.
//       * WARNING — a required branch that never reaches a join (its join-wait
//         is trivially satisfied by termination).
//       * WARNING — a required branch that enters a nested fork before any
//         join (the outer join does not wait for the nested fork's branches;
//         join correlation is by fork parent token).
//   - cycles-allowed policy: cycles are ALLOWED and are not reported as errors.
//     Blocker loops (work -> issue -> blocker -> resolved -> work) are
//     intentional and the engine handles them; only a cycle with NO exit is
//     rejected (it can never complete, which hangs the process).
// ---------------------------------------------------------------------------

import type {
  NodeDefinition,
  ProcessGraph,
  ProcessOutcome,
} from '../../workflow_engine/lib/workflow/types'
import { isSupportedExpression } from '../../workflow_engine/lib/workflow/expressions'

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

/**
 * The engine's actual node dispatch surface (workflow_engine/lib/workflow/engine.ts
 * `_arriveAtNode` / `_executeNodeLeave`): start, end, task, decision, fork,
 * join, timer, command are first-class; `state` is the explicit passthrough.
 * Any OTHER type silently falls through to the passthrough branch — that
 * silent degrade is exactly what ENG-14 rejects at deploy time.
 */
const SUPPORTED_NODE_TYPES: ReadonlySet<string> = new Set([
  'start',
  'end',
  'task',
  'decision',
  'fork',
  'join',
  'timer',
  'command',
  'state',
  'dynamic-fork',
])

/**
 * Types declared in the engine's NodeDefinition union but with NO runtime
 * implementation. They get a targeted diagnostic instead of the generic
 * unsupported-type message.
 */
const UNIMPLEMENTED_NODE_TYPES: ReadonlySet<string> = new Set(['subprocess'])

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

  // ENG-14 — unreachable-node reporting: every node must be reachable from the
  // start node through transitions. A node with no incoming path can never be
  // executed; it is dead weight (a typo'd target or a leftover node).
  if (graph.startNodeId && nodes[graph.startNodeId] !== undefined) {
    const reachable = collectReachableNodes(graph)
    for (const id of nodeIds) {
      if (id !== graph.startNodeId && !reachable.has(id)) {
        errors.push(
          `node '${id}' is unreachable: no path of transitions from start node '${graph.startNodeId}' reaches it`,
        )
      }
    }
  }

  // ENG-14 — fork/join analysis where safely determinable.
  analyzeForkJoin(graph, errors, warnings)

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

  // ENG-14 — unsupported-node diagnostics: reject node types the engine has no
  // handler for instead of letting them silently degrade to passthrough.
  if (!node.type) {
    errors.push(`node '${id}' has no type`)
  } else if (!SUPPORTED_NODE_TYPES.has(node.type)) {
    if (UNIMPLEMENTED_NODE_TYPES.has(node.type)) {
      errors.push(
        `node '${id}' uses type '${node.type}', which is declared in the engine type union but has no runtime implementation — the engine would silently treat it as a passthrough; model the sub-flow inline or add engine support`,
      )
    } else {
      errors.push(
        `node '${id}' has unsupported type '${node.type}' — the engine has no handler for it and would silently treat it as a passthrough`,
      )
    }
  }

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
      } else if (!isSupportedExpression(d.condition)) {
        errors.push(
          `decision node '${id}' has an unsupported condition expression '${d.condition}'`,
        )
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

  if (node.type === 'dynamic-fork') {
    // ENG-FORGE-V9 — data-driven fan-out definition-time contract.
    if (!node.branchCommandType) {
      errors.push(`dynamic-fork node '${id}' has no branch-command-type`)
    }
    if (!node.countVariable) {
      errors.push(`dynamic-fork node '${id}' has no count-variable`)
    }
    if (!node.join) {
      errors.push(`dynamic-fork node '${id}' has no join target`)
    } else if (nodes[node.join] === undefined) {
      errors.push(`dynamic-fork node '${id}' join references missing node '${node.join}'`)
    } else if (nodes[node.join]?.type !== 'join') {
      errors.push(`dynamic-fork node '${id}' join '${node.join}' must reference a <join> node`)
    }
    const min = node.minimum ?? 2
    const max = node.maximum ?? 8
    if (min > max) {
      errors.push(`dynamic-fork node '${id}' minimum (${min}) exceeds maximum (${max})`)
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

// ---------------------------------------------------------------------------
// ENG-14 — static graph analysis (reachability + fork/join).
// ---------------------------------------------------------------------------

/** Every node reachable from the start node by following transitions. */
function collectReachableNodes(graph: ProcessGraph): Set<string> {
  const seen = new Set<string>()
  const stack = [graph.startNodeId]
  while (stack.length > 0) {
    const id = stack.pop()!
    if (seen.has(id)) continue
    seen.add(id)
    for (const t of graph.nodes[id]?.transitions ?? []) {
      if (t.to && !seen.has(t.to)) stack.push(t.to)
    }
    // ENG-FORGE-V9 — a dynamic-fork's branches rejoin at its `join` node, so the
    // join (and everything downstream of it) is reachable from the fork.
    const dynamic = graph.nodes[id]
    if (dynamic?.type === 'dynamic-fork' && dynamic.join && !seen.has(dynamic.join)) {
      stack.push(dynamic.join)
    }
  }
  return seen
}

/**
 * Completion points reachable from a branch entry WITHOUT passing through any
 * completion point (exploration stops at the first one on each path). The
 * engine completes a token at an end node, at a leaf (node with no transitions
 * other than timer/command), at a fork (the parent token completes and children
 * spawn), and at a join (arrival). A required branch whose reachable set
 * contains NONE of these can never complete.
 */
function branchCompletionPoints(
  entry: string,
  nodes: Record<string, NodeDefinition>,
): { ends: string[]; leaves: string[]; forks: string[]; joins: string[] } {
  const seen = new Set<string>()
  const stack = [entry]
  const ends: string[] = []
  const leaves: string[] = []
  const forks: string[] = []
  const joins: string[] = []
  while (stack.length > 0) {
    const id = stack.pop()!
    if (seen.has(id)) continue
    seen.add(id)
    const node = nodes[id]
    if (!node) continue
    if (node.type === 'end') {
      ends.push(id)
      continue
    }
    if (node.type === 'join') {
      joins.push(id)
      continue
    }
    if (node.type === 'fork') {
      forks.push(id)
      continue
    }
    const transitions = node.transitions ?? []
    if (transitions.length === 0 && node.type !== 'timer' && node.type !== 'command') {
      leaves.push(id)
      continue
    }
    for (const t of transitions) {
      if (!seen.has(t.to)) stack.push(t.to)
    }
  }
  return { ends, leaves, forks, joins }
}

/**
 * Fork/join analysis — rejects only what is safely determinable from the
 * graph shape (ENG-14):
 *
 *   ERROR   required branch with no completion point at all (a closed loop
 *           with no exit). The engine's join releases when every required
 *           sibling token is complete; a token that can never complete makes
 *           the join impossible to release — the process would hang.
 *   WARNING required branch that passes through nested fork(s) and never
 *           reaches a join directly (the outer join does not wait for the
 *           nested fork's branches; join correlation is by fork parent token).
 *   WARNING required branch that never reaches a join node (its join-wait is
 *           trivially satisfied by termination).
 *
 * Optional branches (`required=false`) never block a join, so they are not
 * analyzed. Cycles that DO have an exit are intentional blocker loops and are
 * never reported.
 */
function analyzeForkJoin(
  graph: ProcessGraph,
  errors: string[],
  warnings: string[],
): void {
  for (const node of Object.values(graph.nodes ?? {})) {
    if (node.type !== 'fork') continue
    const forkId = node.id
    for (const t of node.transitions ?? []) {
      if (t.required === false) continue // optional branches never block a join
      const points = branchCompletionPoints(t.to, graph.nodes)
      const total =
        points.ends.length + points.leaves.length + points.forks.length + points.joins.length
      if (total === 0) {
        errors.push(
          `required branch '${t.name}' of fork '${forkId}' is a closed loop with no exit: its token can never complete, so the process would hang and any join it feeds can never release (add an exit transition to an end/join node)`,
        )
      } else if (points.joins.length === 0 && points.forks.length > 0) {
        warnings.push(
          `required branch '${t.name}' of fork '${forkId}' passes through nested fork(s) and never reaches a join directly — the outer join does not wait for the nested fork's branches (join correlation is by fork parent token); verify the branch is supposed to bypass the join`,
        )
      } else if (points.joins.length === 0) {
        warnings.push(
          `required branch '${t.name}' of fork '${forkId}' never reaches a join node — the fork-join wait for this branch is trivially satisfied by its termination (it only reaches end/leaf nodes); verify the branch is supposed to bypass the join`,
        )
      }
    }
  }
}
