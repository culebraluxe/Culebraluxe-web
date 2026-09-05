import type { ForgeTransitionDecision } from './forge-transition'
import { parseForgeSdlc } from '../workflow_app/definitions/forge-sdlc'

// ---------------------------------------------------------------------------
// ENG-FORGE-V8 — FORGE_SDLC as the single topology contract of the live Forge
// driver.
//
// FORGE_SDLC-v1.xml (loaded through the same four-layer pipeline as
// RE_supermodel) is the canonical topology. forge-transition.ts stays the
// low-level pure decision reducer, but this module is the seam that binds the
// reducer's outcomes to the XML so they can never silently drift:
//
//   - forgeDecisionNodeId() maps every reducer outcome to its canonical
//     FORGE_SDLC node id;
//   - forgeSdlcTopology() loads + validates the XML once per process;
//   - assertForgeDecisionOnTopology() proves a decision's target node exists in
//     the XML and has the expected type/responsibility (fail closed).
//
// The XML node id IS the Forge state identity (V7). This module holds the one
// explicit identity mapping reducer outcome -> node id and VERIFIES every entry
// against the loaded XML at guard time, so a renamed node, removed task, or
// responsibility change fails the guard instead of drifting.
//
// No database, no packages, no engine. Pure + unit-testable.
// ---------------------------------------------------------------------------

export type ForgeSdlcNodeShape = {
  id: string
  type?: string
  label?: string
  responsibility?: string
  outcome?: string
}

export type ForgeSdlcTopology = {
  key: string
  version: number
  /** Every node id present in the FORGE_SDLC graph. */
  nodeIds: ReadonlySet<string>
  /** Task nodes keyed by id, with their Forge responsibility. */
  tasks: Readonly<Record<string, { id: string; label?: string; responsibility?: string }>>
  /** End-node ids (the only Forge termini). */
  endIds: ReadonlySet<string>
}

let cachedTopology: ForgeSdlcTopology | null = null

/** Re-read + re-validate FORGE_SDLC from disk on the next call (tests only). */
export function resetForgeSdlcTopologyCache(): void {
  cachedTopology = null
}

function structuralInvariants(topology: ForgeSdlcTopology): string[] {
  const problems: string[] = []
  if (topology.key !== 'FORGE_SDLC') {
    problems.push(`FORGE_SDLC key is '${topology.key}', expected 'FORGE_SDLC'`)
  }
  if (topology.version !== 1) {
    problems.push(`FORGE_SDLC version is '${topology.version}', expected 1`)
  }
  if (topology.endIds.size !== 2) {
    problems.push(
      `expected exactly two Forge termini, found ${[...topology.endIds].sort().join(', ') || '(none)'}`,
    )
  } else {
    if (!topology.endIds.has('story_complete')) {
      problems.push('story_complete end node missing')
    }
    if (!topology.endIds.has('forge_hold')) {
      problems.push('forge_hold end node missing')
    }
  }
  // Serial backbone: ready -> scout -> architect -> lead_pre.
  for (const id of ['ready', 'scout', 'architect', 'lead_pre']) {
    if (!topology.nodeIds.has(id)) {
      problems.push(`serial backbone node '${id}' missing`)
    }
  }
  const validRoles = new Set(['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'])
  for (const task of Object.values(topology.tasks)) {
    if (!task.responsibility || !validRoles.has(task.responsibility)) {
      problems.push(
        `task '${task.id}' has responsibility '${task.responsibility ?? '(none)'}', which is not a Forge position`,
      )
    }
  }
  return problems
}

/** Load FORGE_SDLC through the four-layer loader and normalize its topology. */
export function forgeSdlcTopology(): ForgeSdlcTopology {
  if (cachedTopology) return cachedTopology
  const parsed = parseForgeSdlc() // throws if any validation layer fails
  const nodes = parsed.graph.nodes ?? {}
  const tasks: Record<string, { id: string; label?: string; responsibility?: string }> = {}
  const endIds = new Set<string>()
  for (const node of Object.values(nodes) as ForgeSdlcNodeShape[]) {
    if (!node || !node.id) continue
    if (node.type === 'task') {
      tasks[node.id] = {
        id: node.id,
        label: node.label,
        responsibility: node.responsibility,
      }
    } else if (node.type === 'end') {
      endIds.add(node.id)
    }
  }
  const topology: ForgeSdlcTopology = {
    key: parsed.key,
    version: parsed.version,
    nodeIds: new Set(Object.keys(nodes)),
    tasks,
    endIds,
  }
  const problems = structuralInvariants(topology)
  if (problems.length > 0) {
    throw new Error(
      `FORGE_SDLC topology invariant violated:\n${problems.map((p) => `  - ${p}`).join('\n')}`,
    )
  }
  cachedTopology = topology
  return topology
}

/**
 * Map a ForgeTransitionDecision to its canonical FORGE_SDLC node id.
 *
 * This is the ONE explicit identity mapping from reducer outcome to the XML
 * node id. It is verified against the loaded graph by
 * assertForgeDecisionOnTopology() so a stale mapping fails the guard.
 */
export function forgeDecisionNodeId(decision: ForgeTransitionDecision): string {
  switch (decision.action) {
    case 'enqueue-assay':
      return 'assay'
    case 'enqueue-smith':
      return 'smith'
    case 'publish':
      return 'publish'
    case 'complete':
      return 'story_complete'
    case 'hold-human':
      return 'forge_hold'
    case 'retry-same-lane':
      return 'smith'
    case 'enqueue-lead':
      switch (decision.nextPhase) {
        case 'pre':
          return 'lead_pre'
        case 'implement':
          return 'lead_implement'
        case 'post':
          return 'lead_post'
        default:
          throw new Error(
            `enqueue-lead decision has no valid next_phase: ${decision.nextPhase ?? '(none)'}`,
          )
      }
    default:
      throw new Error(`decision action '${decision.action}' has no FORGE_SDLC node`)
  }
}

function expectedShape(decision: ForgeTransitionDecision): {
  type: string
  responsibility?: string
} {
  switch (decision.action) {
    case 'enqueue-assay':
      return { type: 'task', responsibility: 'qa' }
    case 'enqueue-smith':
    case 'retry-same-lane':
      return { type: 'task', responsibility: 'smith' }
    case 'publish':
      return { type: 'task', responsibility: 'dev_ops' }
    case 'enqueue-lead':
      return { type: 'task', responsibility: 'lead' }
    case 'complete':
      return { type: 'end' }
    case 'hold-human':
      return { type: 'end' }
    default:
      return { type: 'task' }
  }
}

/**
 * Prove a reducer decision is representable on the FORGE_SDLC graph: its target
 * node must exist and carry the expected type (and Forge responsibility for
 * task lanes). Throws on any mismatch so the live guard fails closed.
 */
export function assertForgeDecisionOnTopology(
  decision: ForgeTransitionDecision,
  topology: ForgeSdlcTopology = forgeSdlcTopology(),
): void {
  const nodeId = forgeDecisionNodeId(decision)
  if (!topology.nodeIds.has(nodeId)) {
    throw new Error(
      `Forge decision action='${decision.action}' maps to node '${nodeId}', which is not in FORGE_SDLC`,
    )
  }
  const shape = expectedShape(decision)
  if (shape.type === 'end') {
    if (!topology.endIds.has(nodeId)) {
      throw new Error(
        `Forge decision action='${decision.action}' expects node '${nodeId}' to be an end node`,
      )
    }
    return
  }
  const task = topology.tasks[nodeId]
  if (!task) {
    throw new Error(
      `Forge decision action='${decision.action}' expects node '${nodeId}' to be a task node`,
    )
  }
  if (shape.responsibility && task.responsibility !== shape.responsibility) {
    throw new Error(
      `Forge decision action='${decision.action}' expects node '${nodeId}' responsibility '${shape.responsibility}', got '${task.responsibility ?? '(none)'}'`,
    )
  }
}

/**
 * Fail-closed live guard: load + validate FORGE_SDLC once per process. Call at
 * the top of the live Forge orchestration entry points so a broken, drifted, or
 * missing topology contract stops the driver instead of silently mis-routing.
 */
export function ensureForgeSdlcTopology(): ForgeSdlcTopology {
  return forgeSdlcTopology()
}
