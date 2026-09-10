// ---------------------------------------------------------------------------
// Lead SPLIT → smith_split_work handoff.
//
// The XML fork is sibling-only. Each child token must receive ONE accepted
// Lead assignment (not assignments[0], not the whole serial plan).
//
// INDEX CONVENTION — 0-based, matching the engine that creates the children:
//   `workflow_engine/lib/workflow/engine.ts` forks `for (let i = 0; i < count; i++)`
//   and hands each child `splitBranchIndex: i` plus `splitBranch:
//   variables[planVariable][i]` — i.e. index 0 IS the first child. An earlier
//   1-based reading of this field silently failed to resolve the first child.
//   A missing/unparseable index is `null` (not 0) so "absent" is never confused
//   with "the first branch".
//
// Pure + DB-free. Do not enable splitEnabled until the runner calls this
// before launching the child harness.
// ---------------------------------------------------------------------------

import type { LeadAssignment, LeadProposal } from './forge-lead-routing'
import { renderSmithWorkOrders } from './forge-lead-plan'
import type { SmithExecutionContract } from './smith-contract'
import { validateSmithContract } from './smith-contract'

/**
 * The engine's `splitBranchIndex`, as a 0-based array index.
 * Returns null when the field is absent, empty or not a non-negative integer —
 * "absent" must not be read as "branch 0".
 */
export function splitBranchIndexFromForm(
  formData: Record<string, unknown> | null | undefined,
): number | null {
  if (!formData) return null
  const raw = formData.splitBranchIndex ?? formData.splitIndex
  if (raw === null || raw === undefined || raw === '') return null
  const n = Number(raw)
  if (!Number.isInteger(n) || n < 0) return null
  return n
}

/** Map a 0-based branch index onto the accepted proposal. */
export function assignmentForSplitBranch(
  proposal: LeadProposal | null | undefined,
  branchIndex: number | null,
): LeadAssignment | null {
  if (!proposal || proposal.decision !== 'SPLIT' || !proposal.assignments.length) {
    return null
  }
  if (branchIndex == null) return null
  return proposal.assignments[branchIndex] ?? null
}

/**
 * Resolve the ONE assignment a split child owns, reconciled against the slice the
 * engine handed that child.
 *
 * Pure. The runner uses `errors` to HOLD — never to guess. A child that cannot be
 * tied to an accepted assignment must not run at all: handing it the general
 * decomposition directive would let it invent its own scope, which is exactly how
 * a branch gets mangled and a candidate ends up coming from the wrong workspace.
 */
export function splitChildAssignment(input: {
  proposal: LeadProposal | null | undefined
  formData: Record<string, unknown> | null | undefined
}): { assignment: LeadAssignment | null; index: number | null; errors: string[] } {
  const errors: string[] = []
  const index = splitBranchIndexFromForm(input.formData)
  if (index === null) {
    errors.push('split child has no splitBranchIndex on its task form; cannot identify its Lead assignment')
    return { assignment: null, index, errors }
  }
  const assignment = assignmentForSplitBranch(input.proposal, index)
  if (!assignment) {
    errors.push(
      `no accepted Lead assignment for split branch ${index}: the SPLIT proposal is missing, unvalidated, or has fewer assignments than splitCount`,
    )
    return { assignment: null, index, errors }
  }
  // The engine hands each child its own slice (engine.ts: splitBranch = plan[index]).
  // If that slice names a different assignment than the accepted proposal puts at
  // this index, one of the two is stale or off-by-one. Refuse rather than pick a
  // winner — silent divergence here is the mangled-branch incident.
  const slice = input.formData?.splitBranch
  if (slice && typeof slice === 'object' && 'id' in slice) {
    const sliceId = String((slice as { id?: unknown }).id ?? '')
    if (sliceId && sliceId !== assignment.id) {
      errors.push(
        `split branch ${index} carries engine slice '${sliceId}' but the accepted proposal assigns '${assignment.id}' there; refusing rather than guessing`,
      )
    }
  }
  return { assignment, index, errors }
}

export function renderSplitAssignmentWorkOrders(assignment: LeadAssignment): string {
  return [
    `SPLIT assignment ${assignment.id} — execute ONLY this assignment. Do not edit sibling surfaces.`,
    assignment.reasoning ? `Lead reasoning: ${assignment.reasoning}` : '',
    renderSmithWorkOrders(assignment.plan),
  ]
    .filter(Boolean)
    .join('\n')
}

export function smithContractFromAssignment(input: {
  storyId: string
  nodeId: string
  attempt: number
  assignment: LeadAssignment
}): { contract: SmithExecutionContract; errors: string[] } {
  const surfaces = input.assignment.plan.chunks.flatMap((c) => c.surface)
  const proofs = input.assignment.plan.chunks.map((c) => c.proof)
  const contract: SmithExecutionContract = {
    identity: {
      storyId: input.storyId,
      nodeId: input.nodeId,
      attempt: input.attempt,
      owner: input.assignment.id,
    },
    objective: input.assignment.reasoning,
    requiredInputs: input.assignment.evidenceRefs,
    allowedScope: surfaces,
    prohibitedScope: [],
    expectedOutputs: input.assignment.plan.chunks.map((c) => c.outcome),
    requiredEvidence: proofs,
    dependsOn: input.assignment.dependsOn,
  }
  return { contract, errors: validateSmithContract(contract).errors }
}
