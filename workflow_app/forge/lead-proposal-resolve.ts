/**
 * ONE seat for the Lead's routing decision.
 *
 * Both the phase agent (LeadAgent.collect) and the role runner resolve the Lead's
 * proposal through this function, so the decision cannot be evaluated twice with
 * different inputs — the defect where collect refused a proposal and the runner then
 * re-reviewed it from text plus DB and could ACCEPT what collect had just rejected.
 *
 * AUTHORITY ORDER: the recorded FIELDS win, the reply's JSON is the fallback.
 *
 * The decision travels in `forge_role_contract` / `forge_role_plan` (migrations 170,
 * 171). A marker prefix going missing must never cost a routing decision the engine
 * already holds — that is exactly what cost 18 minutes on 2026-09-13. The reply still
 * supplies assignment detail when a route needs a plan (chunks, surfaces, proofs that
 * no 8-column row should hold); what the fields guarantee is the DECISION itself.
 *
 * A HOLD recorded in fields is clean and authoritative: no assignments, no merge
 * checks, because the reviewer refuses a HOLD that dispatches anything.
 */
import type { ForgeRoleContract } from '../../db/forge-role-contract'
import type { ForgeRolePlan } from '../../db/forge-role-plan'
import {
  parseLeadRouting,
  reviewLeadProposal,
  type RoutingContext,
  type RoutingReview,
} from './forge-lead-routing'

/** Merge the recorded decision fields over whatever the reply said. */
export function leadProposalFromFields(
  parsed: unknown,
  contract: ForgeRoleContract | null,
  plan: ForgeRolePlan | null,
): unknown {
  if (!contract?.decision) return parsed

  if (contract.decision === 'HOLD') {
    const base =
      parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        ? (parsed as Record<string, unknown>)
        : {}
    return {
      version: 1,
      decision: 'HOLD',
      size: contract.size ?? (base.size as string) ?? 'SMALL',
      sizeReason: contract.sizeReason ?? contract.reason ?? 'recorded in fields',
      reason: contract.reason ?? contract.sizeReason ?? 'recorded in fields',
      assignments: [],
      mergeChecks: [],
    }
  }

  const base =
    plan && plan.assignments.length > 0
      ? { version: 1, assignments: plan.assignments, size: plan.size }
      : parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        ? { ...(parsed as Record<string, unknown>) }
        : null
  if (!base) {
    // No parseable plan and the fields ask for a route that NEEDS one. Say exactly
    // that rather than inventing a plan the model never wrote.
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

/**
 * Resolve and REVIEW the Lead proposal: fields first, reply as fallback.
 *
 * `parsedOverride` exists for callers that already hold a validated proposal (the
 * runner's accepted-routing recovery path); ordinary callers pass only `raw`.
 */
export function resolveLeadProposal(input: {
  raw: string
  contract: ForgeRoleContract | null
  plan: ForgeRolePlan | null
  context: RoutingContext
}): RoutingReview {
  return reviewLeadProposal(
    leadProposalFromFields(parseLeadRouting(input.raw), input.contract, input.plan),
    input.context,
  )
}
