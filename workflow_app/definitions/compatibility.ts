// ---------------------------------------------------------------------------
// ENG-12 — Workflow Definition Versioning / Compatibility diagnostics.
//
// Formalizes the existing V1 versioning policy and makes running-instance
// compatibility explicit, deterministic, and testable:
//
//   1. deployed definitions are immutable historical artifacts
//   2. a running instance stays pinned to its exact definition_id forever
//   3. NEW instances use the newly deployed version
//   4. ALL deployed versions remain available (never deleted, never rewritten)
//   5. removed or renamed nodes affect ONLY new definitions — a running
//      instance continues on the exact graph it started with
//   6. rollback means deploying a prior graph as a NEW version number
//   7. cross-version instance migration is UNSUPPORTED by design
//
// This module is pure (no database/Neon import), like version-policy.ts, so
// the diagnostics are testable without touching the deployment client.
//
// Division of labor with version-policy.ts:
//   - version-policy.ts  decides whether a (key, version) row may be WRITTEN
//     (new / replaceable / immutable) — the deploy-time gate.
//   - this module        explains WHAT changed between two deployed graphs and
//     WHY running instances are unaffected — the operational diagnostic.
//
// Deliberately NOT a migration framework: versions are never rewritten and
// instances are never re-pointed. Completing or cancelling a running instance
// is the only path off its pinned version.
// ---------------------------------------------------------------------------

import type { NodeDefinition, ProcessGraph } from '../../workflow_engine/lib/workflow/types'

export interface NodeRename {
  nodeId: string
  from: string
  to: string
}

export interface NodeChange {
  nodeId: string
  changes: string[]
}

export interface DefinitionDiff {
  /** True when the executable graph (nodes + start node) is unchanged. */
  identical: boolean
  addedNodeIds: string[]
  removedNodeIds: string[]
  renamedNodes: NodeRename[]
  changedNodes: NodeChange[]
  changedStartNode: boolean
  /** Presentation-only metadata (portal timeline) — never affects execution. */
  displayOrderChanged: boolean
}

export interface CompatibilityDiagnostics {
  diff: DefinitionDiff
  /** Deterministic, human-readable summary of what changed. */
  summary: string[]
  /** Running-instance impact statements (the V1 policy, made explicit). */
  runningInstanceImpact: string[]
  /** Rollback detection: next re-deploys a prior graph as a new version. */
  rollback: { isRollback: boolean; detail: string }
  /** The explicit cross-version migration statement. */
  migrationUnsupported: string
}

const MIGRATION_UNSUPPORTED =
  'cross-version instance migration is UNSUPPORTED: a running instance never switches to a newer definition version; completing or cancelling the instance is the only path off its pinned version'

const RUNNING_INSTANCE_IMPACT = [
  'running instances are pinned to their exact definition_id forever — never re-pointed to a newer version',
  'removed or renamed nodes in a new version affect only NEW instances; a running instance continues on the graph it started with',
  'all deployed versions remain available — nothing is deleted and no version is rewritten in place',
  'rollback means deploying a prior graph as a NEW version number — never mutating the prior version',
]

/** Stable canonical JSON (object keys sorted; undefined values dropped). */
function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map((v) => canonicalJson(v)).join(',')}]`
  }
  if (value !== null && typeof value === 'object') {
    const entries = Object.keys(value as Record<string, unknown>)
      .sort()
      .filter((k) => (value as Record<string, unknown>)[k] !== undefined)
      .map((k) => `${JSON.stringify(k)}:${canonicalJson((value as Record<string, unknown>)[k])}`)
    return `{${entries.join(',')}}`
  }
  return JSON.stringify(value)
}

/**
 * Full stored-definition equality (canonical JSON, key order insensitive).
 * Used by the deploy layer to detect a byte-identical duplicate redeploy of an
 * existing (key, version) row. Includes displayOrder because a redeploy that
 * changes it would change the stored definition row.
 */
export function graphsEqual(a: ProcessGraph, b: ProcessGraph): boolean {
  return canonicalJson(a) === canonicalJson(b)
}

/** Executable-graph identity: nodes + start node (displayOrder excluded). */
function graphSignature(graph: ProcessGraph): string {
  return canonicalJson({ nodes: graph.nodes ?? {}, startNodeId: graph.startNodeId })
}

/** Transition set identity for a node (name -> target pairs, in declaration order). */
function transitionsSignature(node: NodeDefinition): string {
  return canonicalJson((node.transitions ?? []).map((t) => [t.name, t.to, t.required ?? true]))
}

/**
 * Compute the deterministic diff between two deployed graphs of the SAME
 * logical key. Node id is the workflow state identity, so:
 *   - a node present in next but not previous is ADDED
 *   - a node present in previous but not next is REMOVED
 *   - a node with the same id but a different name is RENAMED (presentation
 *     metadata; the state identity is unchanged, so it is safe for running
 *     instances)
 *   - a node with the same id but different type/transitions/command/outcome
 *     is CHANGED structurally (also only affects new instances)
 */
export function diffProcessGraphs(previous: ProcessGraph, next: ProcessGraph): DefinitionDiff {
  const prevNodes = previous.nodes ?? {}
  const nextNodes = next.nodes ?? {}
  const prevIds = new Set(Object.keys(prevNodes))
  const nextIds = new Set(Object.keys(nextNodes))

  const addedNodeIds = Object.keys(nextNodes).filter((id) => !prevIds.has(id)).sort()
  const removedNodeIds = Object.keys(prevNodes).filter((id) => !nextIds.has(id)).sort()

  const renamedNodes: NodeRename[] = []
  const changedNodes: NodeChange[] = []
  for (const id of Object.keys(nextNodes)) {
    if (!prevIds.has(id)) continue
    const prev = prevNodes[id]
    const next = nextNodes[id]
    const fromName = prev.name ?? id
    const toName = next.name ?? id
    if (fromName !== toName) {
      renamedNodes.push({ nodeId: id, from: fromName, to: toName })
    }
    const changes: string[] = []
    if (prev.type !== next.type) changes.push(`type ${prev.type} -> ${next.type}`)
    if (transitionsSignature(prev) !== transitionsSignature(next)) {
      changes.push('transitions changed')
    }
    if (prev.commandType !== next.commandType) {
      changes.push(`commandType ${prev.commandType ?? '(none)'} -> ${next.commandType ?? '(none)'}`)
    }
    if ((prev.outcome ?? 'completed') !== (next.outcome ?? 'completed')) {
      changes.push(`outcome ${prev.outcome ?? 'completed'} -> ${next.outcome ?? 'completed'}`)
    }
    if ((prev.decisions ?? []).length !== (next.decisions ?? []).length) {
      changes.push('decision rules changed')
    }
    if (changes.length > 0) changedNodes.push({ nodeId: id, changes })
  }

  const displayOrderChanged =
    canonicalJson(previous.displayOrder ?? []) !== canonicalJson(next.displayOrder ?? [])

  const identical =
    addedNodeIds.length === 0 &&
    removedNodeIds.length === 0 &&
    renamedNodes.length === 0 &&
    changedNodes.length === 0 &&
    previous.startNodeId === next.startNodeId

  return {
    identical,
    addedNodeIds,
    removedNodeIds,
    renamedNodes,
    changedNodes,
    changedStartNode: previous.startNodeId !== next.startNodeId,
    displayOrderChanged,
  }
}

/**
 * True when `next` re-deploys the exact executable graph of `previous` — the
 * rollback-as-new-version shape. Identity is the executable graph (nodes +
 * start node); display-order changes are presentation-only and do not make a
 * rollback deployment a non-rollback.
 */
export function isRollbackDeployment(previous: ProcessGraph, next: ProcessGraph): boolean {
  return graphSignature(previous) === graphSignature(next)
}

/**
 * Build the explicit compatibility diagnostic report for deploying `next`
 * (a new version of the same logical key) when `previous` is already deployed.
 * Deterministic and pure; never throws for graph-shaped input.
 */
export function compatibilityDiagnostics(
  previous: ProcessGraph,
  next: ProcessGraph,
  opts: { key?: string; previousVersion?: number; nextVersion?: number } = {},
): CompatibilityDiagnostics {
  const diff = diffProcessGraphs(previous, next)

  const summary: string[] = []
  if (diff.identical) {
    summary.push(
      `no executable-graph changes between the previous version and this version${opts.nextVersion ? ` (v${opts.nextVersion})` : ''}`,
    )
  } else {
    if (diff.changedStartNode) {
      summary.push(`start node changed: ${previous.startNodeId} -> ${next.startNodeId}`)
    }
    if (diff.addedNodeIds.length > 0) {
      summary.push(`added node(s): ${diff.addedNodeIds.join(', ')}`)
    }
    if (diff.removedNodeIds.length > 0) {
      summary.push(`removed node(s): ${diff.removedNodeIds.join(', ')}`)
    }
    if (diff.renamedNodes.length > 0) {
      summary.push(
        `renamed node(s): ${diff.renamedNodes
          .map((r) => `${r.nodeId} ('${r.from}' -> '${r.to}')`)
          .join(', ')}`,
      )
    }
    for (const c of diff.changedNodes) {
      summary.push(`changed node '${c.nodeId}': ${c.changes.join('; ')}`)
    }
    if (diff.displayOrderChanged) {
      summary.push('display order changed (presentation only — never affects execution)')
    }
  }

  const keyLabel = opts.key ? `'${opts.key}'` : 'this key'
  const rollbackDetail = diff.identical
    ? `this version re-deploys the exact executable graph of the previous version${opts.previousVersion ? ` (v${opts.previousVersion})` : ''} — a rollback-as-new-version deployment. It MUST be deployed as a NEW version number${opts.nextVersion ? ` (v${opts.nextVersion})` : ''} for ${keyLabel}; the prior version remains immutable and available.`
    : `not a rollback: the executable graph differs from the previous version (${diff.addedNodeIds.length} added, ${diff.removedNodeIds.length} removed, ${diff.renamedNodes.length} renamed, ${diff.changedNodes.length} structurally changed).`

  return {
    diff,
    summary,
    runningInstanceImpact: RUNNING_INSTANCE_IMPACT,
    rollback: { isRollback: diff.identical, detail: rollbackDetail },
    migrationUnsupported: MIGRATION_UNSUPPORTED,
  }
}
