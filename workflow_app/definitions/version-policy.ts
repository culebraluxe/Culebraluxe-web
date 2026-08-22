// ---------------------------------------------------------------------------
// Definition version / redeploy contract (Story 133, formalized by ENG-12).
//
// Deployed definitions are immutable historical artifacts. A running instance
// is pinned to its `definition_id`/`version` forever. Changing the workflow
// means deploying a NEW version under the SAME logical key — never mutating a
// deployed version that has already executed.
//
// This module is pure (no database/Neon import) so the policy can be tested
// without touching the deployment client.
//
// ENG-12 additions:
//   - `classifyDeploy` is the single explicit deploy decision table used by
//     the deployment service: insert (new), update (replaceable draft /
//     duplicate redeploy), or reject (immutable — has instances, so even a
//     byte-identical redeploy is refused: a version that has executed is never
//     written again).
//   - `deploymentCompatibility` composes the version-policy decision with the
//     pure compatibility diagnostics from `compatibility.ts` so operators see
//     BOTH what may be written AND what changed for running instances.
// ---------------------------------------------------------------------------

import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import { graphsEqual, compatibilityDiagnostics } from './compatibility'
import type { CompatibilityDiagnostics } from './compatibility'

export type DefinitionVersionPolicy =
  | { kind: 'new' }
  | { kind: 'replaceable' }
  | { kind: 'immutable' }

/**
 * Classify whether a (key, version) row may be written:
 *   - no row exists          -> new          (plain insert)
 *   - row exists, no instances -> replaceable (safe to iterate a draft version)
 *   - row exists, has instances -> immutable  (must deploy a NEW version)
 */
export function definitionVersionPolicy(
  rowExists: boolean,
  instanceCount: number,
): DefinitionVersionPolicy {
  if (!rowExists) return { kind: 'new' }
  return instanceCount > 0 ? { kind: 'immutable' } : { kind: 'replaceable' }
}

export const IMMUTABLE_DEFINITION_ERROR =
  'Refusing to replace a deployed process-definition version that already has instances; deploy a new version instead.'

// ---------------------------------------------------------------------------
// ENG-12 — explicit deploy decision table (the deploy service executes this).
// ---------------------------------------------------------------------------

export type DeployDecision =
  | { action: 'insert'; created: true; reason: 'new' }
  | { action: 'update'; created: false; duplicate: boolean; reason: 'replaceable' }
  | { action: 'reject'; created: false; reason: 'immutable'; message: string }

/**
 * The full deploy decision for a (key, version) row:
 *
 *   no row                         -> insert (plain new version)
 *   row, no instances, same graph  -> update, duplicate: true
 *                                    (idempotent duplicate redeploy of a draft)
 *   row, no instances, new graph   -> update, duplicate: false
 *                                    (safe draft iteration)
 *   row, HAS instances, any graph  -> reject
 *                                    (immutable: a version that has executed is
 *                                    NEVER written again — this is the explicit
 *                                    rejection of in-place mutation, and it
 *                                    holds even for a byte-identical redeploy)
 *
 * `previousGraph` is the definition currently stored in the row (null when the
 * row does not exist). `incomingGraph` is the graph being deployed.
 */
export function classifyDeploy(
  rowExists: boolean,
  instanceCount: number,
  previousGraph: ProcessGraph | null,
  incomingGraph: ProcessGraph,
): DeployDecision {
  if (!rowExists) return { action: 'insert', created: true, reason: 'new' }
  if (instanceCount > 0) {
    return {
      action: 'reject',
      created: false,
      reason: 'immutable',
      message: IMMUTABLE_DEFINITION_ERROR,
    }
  }
  const duplicate = previousGraph !== null && graphsEqual(previousGraph, incomingGraph)
  return { action: 'update', created: false, duplicate, reason: 'replaceable' }
}

export type DeploymentCompatibilityReport = {
  /** What may be written to the (key, version) row. */
  decision: DeployDecision
  /** What changed vs. the previous deployed graph (null when no previous row). */
  diagnostics: CompatibilityDiagnostics | null
}

/**
 * One operator-facing report for a deploy attempt: the explicit deploy
 * decision PLUS the pure compatibility diagnostics (what changed and why
 * running instances are unaffected). Pure — the deploy service renders this.
 */
export function deploymentCompatibility(
  rowExists: boolean,
  instanceCount: number,
  previousGraph: ProcessGraph | null,
  incomingGraph: ProcessGraph,
  opts: { key?: string; previousVersion?: number; nextVersion?: number } = {},
): DeploymentCompatibilityReport {
  const decision = classifyDeploy(rowExists, instanceCount, previousGraph, incomingGraph)
  const diagnostics = previousGraph
    ? compatibilityDiagnostics(previousGraph, incomingGraph, opts)
    : null
  return { decision, diagnostics }
}
