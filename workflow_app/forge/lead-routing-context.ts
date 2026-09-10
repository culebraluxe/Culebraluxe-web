// ---------------------------------------------------------------------------
// Trusted hydration for LEAD routing (Astra handoff, 2026-09-10).
//
// `RoutingContext` is the ONLY input the routing validator trusts besides the
// model's proposal: findings, named evidence references, the frozen proof
// commands, and runtime capability. The model must never be able to set any of
// them, so this module derives them from durable story/task state — never from
// model prose.
//
// Runtime capability is deliberately conservative today:
//   splitEnabled: false  — the SPLIT lane is unproven (mangled branch, candidate
//                          captured from the wrong workspace; see MEMORY
//                          2026-09-10). Declaring it unavailable makes a Lead
//                          SPLIT proposal get CORRECTED instead of silently
//                          routed into a broken lane.
//   maxSmiths: 1         — the executor runs splitConcurrency 1.
// ---------------------------------------------------------------------------
import { parseAssayCommands } from '../../agent-runtime/assay-plan'
import { parseLeadRouting, reviewLeadProposal, type LeadProposal, type RoutingContext } from './forge-lead-routing'
import type { ForgeGateEvidence } from './forge-facts'

/** Story fields the Lead handoff actually receives. */
export type LeadRoutingStoryFields = {
  id: string
  goal?: string | null
  acceptanceCriteria?: string | null
  architectBrief?: string | null
  contextRefs?: string | null
  assayCommands?: string | null
  packetSha?: string | null
}

export type LeadRoutingCapabilities = {
  splitEnabled: boolean
  maxSmiths: number
}

/**
 * The named references the Lead is actually handed. These are the only strings a
 * proposal may cite in `evidenceRefs`; they are injected into the directive in the
 * same JSON the model answers against, so the vocabulary is self-consistent.
 */
export function leadEvidenceRefs(story: LeadRoutingStoryFields): string[] {
  const refs: string[] = []
  if (story.architectBrief?.trim()) refs.push('architect_brief')
  if (story.contextRefs?.trim()) refs.push('context_refs')
  if (story.acceptanceCriteria?.trim()) refs.push('acceptance_criteria')
  if (story.goal?.trim()) refs.push('story_goal')
  if (story.packetSha?.trim()) refs.push(`packet:${story.packetSha.trim()}`)
  return refs
}

export function buildLeadRoutingContext(input: {
  story: LeadRoutingStoryFields
  findings?: ForgeGateEvidence['findings']
  capabilities: LeadRoutingCapabilities
}): RoutingContext {
  return {
    findings: (input.findings ?? []).map((f) => ({
      id: f.id,
      required: Boolean(f.required),
      ...(f.hint ? { hint: f.hint } : {}),
      seams: f.seams ?? [],
    })),
    evidenceRefs: leadEvidenceRefs(input.story),
    splitEnabled: input.capabilities.splitEnabled,
    maxSmiths: input.capabilities.maxSmiths,
    // The frozen story acceptance is the ONLY legal proof vocabulary.
    allowedProofs: parseAssayCommands(input.story.assayCommands),
  }
}

/**
 * Recover the accepted routing decision from a prior Lead run.
 *
 * The Lead's own run notes carry the single `LEAD_ROUTING:` line, so the accepted
 * proposal is durable in the same place the legacy `LEAD_PLAN` was — it is simply
 * re-VALIDATED against trusted context here rather than trusted blindly. Returns
 * null when no prior proposal exists or the last one does not validate.
 */
export function findLatestAcceptedLeadRouting(
  runs: Array<{ runType?: string | null; notes?: string | null }> | null | undefined,
  context: RoutingContext,
): LeadProposal | null {
  if (!runs || runs.length === 0) return null
  for (let i = runs.length - 1; i >= 0; i--) {
    const run = runs[i]
    if (run.runType !== 'lead' || !(run.notes ?? '').includes('LEAD_ROUTING:')) continue
    const review = reviewLeadProposal(parseLeadRouting(run.notes ?? ''), context)
    if (review.ok) return review.proposal
    return null
  }
  return null
}
