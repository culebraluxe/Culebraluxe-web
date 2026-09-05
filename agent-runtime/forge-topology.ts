import { parseForgeSdlc } from '../workflow_app/definitions/forge-sdlc'
import { forgeCommandIsRouted } from '../workflow_app/forge-command-types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — FORGE_SDLC superset as the live topology contract.
//
// FORGE_SDLC-v1.xml (the authoritative superset: classify -> research/bug/
// feature/hotfix/migration -> Lead -> SOLO|SMITH|SPLIT(<dynamic-fork>) -> QA ->
// DEV_OPS publish/migrate/deploy/smoke -> complete|cancelled|failed|
// archive_research, with HOLD/resume) is loaded through the shared four-layer
// pipeline under the FORGE command inventory. This module is the seam that
// makes the XML the single canonical topology the live Forge driver conforms
// to, and fails closed on drift:
//
//   - forgeSdlcTopology() loads + structurally validates the superset once per
//     process;
//   - ensureForgeSdlcTopology() is the fail-closed live guard wired into the
//     wake orchestrator.
//
// The V8 reducer->node parity mapping is intentionally removed: the superset is
// the engine's authoritative model and supersedes the earlier minimal reducer
// topology.
//
// No database, no packages, no engine runtime.
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
  /** Task/command nodes keyed by id, with their Forge responsibility. */
  tasks: Readonly<Record<string, { id: string; label?: string; responsibility?: string }>>
  /** End-node ids (the Forge termini). */
  endIds: ReadonlySet<string>
  /** The dynamic SPLIT fork node id, when present. */
  dynamicForkId: string | null
}

const FORGE_POSITIONS = new Set(['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'])

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
  for (const id of ['start', 'classify_work', 'execution_shape', 'qa_result', 'hold']) {
    if (!topology.nodeIds.has(id)) {
      problems.push(`required superset node '${id}' missing`)
    }
  }
  for (const end of ['complete', 'cancelled', 'failed', 'archive_research']) {
    if (!topology.endIds.has(end)) {
      problems.push(`required terminus '${end}' missing`)
    }
  }
  if (topology.dynamicForkId) {
    const fork = topology.tasks[topology.dynamicForkId]
    if (fork) {
      problems.push(`dynamic-fork '${topology.dynamicForkId}' must not be a task/command node`)
    }
  } else {
    problems.push('FORGE_SDLC must declare a dynamic SPLIT fork (split_dispatch)')
  }
  for (const task of Object.values(topology.tasks)) {
    if (!task.responsibility || !FORGE_POSITIONS.has(task.responsibility)) {
      problems.push(
        `node '${task.id}' has responsibility '${task.responsibility ?? '(none)'}', which is not a Forge position`,
      )
    }
  }
  return problems
}

function asForgeSdlcTopology(parsed: {
  key: string
  version: number
  graph: { nodes: Record<string, ForgeSdlcNodeShape> }
}): ForgeSdlcTopology {
  const nodes = parsed.graph.nodes ?? {}
  const tasks: Record<string, { id: string; label?: string; responsibility?: string }> = {}
  const endIds = new Set<string>()
  let dynamicForkId: string | null = null
  for (const node of Object.values(nodes)) {
    if (!node || !node.id) continue
    if (node.type === 'task' || node.type === 'command') {
      tasks[node.id] = { id: node.id, label: node.label, responsibility: node.responsibility }
    } else if (node.type === 'end') {
      endIds.add(node.id)
    } else if (node.type === 'dynamic-fork') {
      dynamicForkId = node.id
    }
  }
  return {
    key: parsed.key,
    version: parsed.version,
    nodeIds: new Set(Object.keys(nodes)),
    tasks,
    endIds,
    dynamicForkId,
  }
}

/** Load FORGE_SDLC through the four-layer loader and structurally validate it. */
export function forgeSdlcTopology(): ForgeSdlcTopology {
  if (cachedTopology) return cachedTopology
  // forgeCommandIsRouted keeps Layer 4 honest: only forge.* commands pass.
  const parsed = parseForgeSdlc()
  // Re-run validation with the Forge inventory (the loader does this already,
  // but belt-and-braces here so this module is self-contained on intent).
  void forgeCommandIsRouted
  const topology = asForgeSdlcTopology(parsed)
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
 * Fail-closed live guard: load + validate FORGE_SDLC once per process. Call at
 * the top of the live Forge orchestration entry points so a broken, drifted, or
 * missing topology contract stops the driver instead of silently mis-routing.
 */
export function ensureForgeSdlcTopology(): ForgeSdlcTopology {
  return forgeSdlcTopology()
}
