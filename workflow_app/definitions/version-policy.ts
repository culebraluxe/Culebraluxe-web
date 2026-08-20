// ---------------------------------------------------------------------------
// Definition version / redeploy contract (Story 133).
//
// Deployed definitions are immutable historical artifacts. A running instance
// is pinned to its `definition_id`/`version` forever. Changing the workflow
// means deploying a NEW version under the SAME logical key — never mutating a
// deployed version that has already executed.
//
// This module is pure (no database/Neon import) so the policy can be tested
// without touching the deployment client.
// ---------------------------------------------------------------------------

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
