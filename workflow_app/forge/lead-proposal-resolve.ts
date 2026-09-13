/**
 * THE LEAD'S ROUTING DECISION, FROM ROWS ONLY.
 *
 * One seat: the role runner resolves through this function. `LeadAgent.collect` is a
 * deliberate no-op for PRE, so there is exactly one place a Lead decision can come from
 * and no second evaluator that could accept what the first refused.
 *
 * THE REPLY IS NOT AN INPUT. There is no `raw` parameter, on purpose: chat JSON cannot
 * set `leadDecision`. That is not a preference, it is the rip. "Prefers rows" let a
 * model beat the database with a well-worded line, and on 2026-09-13 the model was
 * taught BOTH channels at once — the task line said "run forge-handoff.mjs", the routing
 * directive said "emit LEAD_ROUTING" — so `forge_role_contract` stayed empty on every
 * run and the decision always arrived as a marker.
 *
 * The marker parsers still exist for Architect and Scout findings, which have no field
 * writer yet. They are not reachable from here.
 *
 * A decision that was never written to rows is a HOLD with a reason the model can act
 * on, never a route.
 */
import type { ForgeRoleContract } from '../../db/forge-role-contract'
import type { ForgeRolePlan } from '../../db/forge-role-plan'
import { reviewLeadProposal, type RoutingContext, type RoutingReview } from './forge-lead-routing'

/** The proposal the recorded fields describe, or the honest absence of one. */
export function leadProposalFromFields(
  contract: ForgeRoleContract | null,
  plan: ForgeRolePlan | null,
): unknown {
  if (!contract?.decision) return null

  if (contract.decision === 'HOLD') {
    return {
      version: 1,
      decision: 'HOLD',
      size: contract.size ?? 'SMALL',
      sizeReason: contract.sizeReason ?? contract.reason ?? 'recorded in fields',
      reason: contract.reason ?? contract.sizeReason ?? 'recorded in fields',
      assignments: [],
      mergeChecks: [],
    }
  }

  const base =
    plan && plan.assignments.length > 0
      ? { version: 1, assignments: plan.assignments, size: plan.size }
      : null
  if (!base) {
    // The decision asks for a route that NEEDS a plan and no chunk rows were written.
    // Say exactly that rather than inventing a plan the model never wrote: the reviewer
    // refuses it, the self-heal reprompt names the missing rows, and the run HOLDs.
    return {
      version: 1,
      decision: contract.decision,
      size: contract.size ?? 'SMALL',
      sizeReason: contract.sizeReason ?? 'recorded in fields',
      reason: contract.reason ?? contract.sizeReason ?? 'recorded in fields',
      assignments: [],
      mergeChecks: contract.mergeChecks,
    }
  }
  return {
    ...base,
    decision: contract.decision,
    ...(contract.size ? { size: contract.size } : {}),
    ...(contract.sizeReason ? { sizeReason: contract.sizeReason } : {}),
    ...(contract.reason ? { reason: contract.reason } : {}),
    ...(contract.mergeChecks.length > 0 ? { mergeChecks: contract.mergeChecks } : {}),
  }
}

/** Resolve and REVIEW the recorded decision. No reply, by construction. */
export function resolveLeadProposal(input: {
  contract: ForgeRoleContract | null
  plan: ForgeRolePlan | null
  context: RoutingContext
}): RoutingReview {
  return reviewLeadProposal(leadProposalFromFields(input.contract, input.plan), input.context)
}
