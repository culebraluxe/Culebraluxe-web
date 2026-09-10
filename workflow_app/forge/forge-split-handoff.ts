// ---------------------------------------------------------------------------
// Lead SPLIT → smith_split_work handoff.
//
// The XML fork is sibling-only. Each child token must receive ONE accepted
// Lead assignment (not assignments[0], not the whole serial plan).
// formData.splitBranchIndex is 1-based when present; 0 means "unset".
//
// Pure + DB-free. Do not enable splitEnabled until the runner calls this
// before launching the child harness.
// ---------------------------------------------------------------------------

import type { LeadAssignment, LeadProposal } from './forge-lead-routing'
import { renderSmithWorkOrders } from './forge-lead-plan'
import type { SmithExecutionContract } from './smith-contract'
import { validateSmithContract } from './smith-contract'

export function splitBranchIndexFromForm(
  formData: Record<string, unknown> | null | undefined,
): number | null {
  if (!formData) return null
  const raw = formData.splitBranchIndex ?? formData.splitIndex
  const n = Number(raw)
  if (!Number.isInteger(n) || n < 1) return null
  return n
}

/**
 * Map a 1-based branch index onto the accepted proposal.
 * Falls back to assignment order when ids are a-1 / lane-1 style.
 */
export function assignmentForSplitBranch(
  proposal: LeadProposal | null | undefined,
  branchIndex: number | null,
): LeadAssignment | null {
  if (!proposal || proposal.decision !== 'SPLIT' || !proposal.assignments.length) {
    return null
  }
  if (branchIndex == null) return null
  const byPosition = proposal.assignments[branchIndex - 1]
  if (byPosition) return byPosition
  const asId = String(branchIndex)
  return (
    proposal.assignments.find((a) => a.id === asId || a.id.endsWith(`-${asId}`)) ??
    null
  )
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
