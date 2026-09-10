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
import { proposalValidShape } from './forge-lead-routing'
import { DEFAULT_FORGE_TEAM } from '../../agent-runtime/team'
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

/** Runtime capability — supplied by the trusted runtime, never by the model. */
export type LeadRoutingCapabilities = {
  splitEnabled: boolean
  maxSmiths: number
}

/** Bounded content extras injected alongside the trusted refs. */
export type LeadRoutingContentExtras = {
  /**
   * ACTUAL contents of the handoff material, not just its names — a real lead reads
   * the work package, not a label pointing at it. Truncated, bounded.
   */
  evidence?: { architectContract?: string; scoutContext?: string }
  /** What LEAD and Smith can actually do right now (single source: the team map). */
  workers?: { lead: string; smith: string }
}

/** The context injected into the LEAD PRE directive. */
export type LeadRoutingContext = RoutingContext & LeadRoutingContentExtras

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
}): LeadRoutingContext {
  const content = (value: string | null | undefined, cap: number): string | undefined => {
    const text = (value ?? '').trim()
    if (!text) return undefined
    return text.length > cap ? `${text.slice(0, cap)}\n[bounded to ${cap} characters]` : text
  }
  const architectContract = content(input.story.architectBrief, CONTENT_CAP)
  const scoutContext = content(input.story.contextRefs, CONTENT_CAP)
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
    // A real lead reads the work, not a pointer to it. Contents are bounded so the
    // directive stays inside the model's context budget.
    ...(architectContract || scoutContext
      ? {
          evidence: {
            ...(architectContract ? { architectContract } : {}),
            ...(scoutContext ? { scoutContext } : {}),
          },
        }
      : {}),
    // What the workers actually are — single source of truth is the team map, so the
    // model reasons about the real configured capability instead of a guess.
    workers: workerSummary(),
  }
}

/** Bounded content budget for handoff material injected into the directive. */
export const CONTENT_CAP = 6000

/** Describe the configured LEAD/Smith capability from the team map. */
function workerSummary(): { lead: string; smith: string } {
  const lead = DEFAULT_FORGE_TEAM.assignments.lead
  const smith = DEFAULT_FORGE_TEAM.assignments.smith
  const upgrade = smith.upgrade ? `; escalates to ${smith.upgrade.profile} on failure` : ''
  return {
    lead: `${lead.profile} via ${lead.harnessId} (position ${lead.position})`,
    smith: `${smith.profile} via ${smith.harnessId}${upgrade}`,
  }
}

/**
 * The accepted routing decision for this story.
 *
 * The DURABLE accepted proposal wins: it was validated once, at acceptance, and it
 * is a business fact from that moment on. Re-deriving it from run notes and
 * re-validating against the live context is a LEGACY FALLBACK only — the context
 * mutates while a story runs (findings are story-global and are rewritten as roles
 * report), which on 2026-09-10 left a live fan-out unable to recover its own
 * accepted decision (`no accepted Lead assignment for split branch 0`).
 */
export function acceptedLeadRouting(input: {
  /** Durable accepted proposal (forge_workflow_evidence.lead_routing). */
  durable?: unknown
  runs?: Array<{ runType?: string | null; notes?: string | null }> | null
  context: RoutingContext
}): LeadProposal | null {
  if (proposalValidShape(input.durable)) return input.durable
  return findLatestAcceptedLeadRouting(input.runs, input.context)
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
