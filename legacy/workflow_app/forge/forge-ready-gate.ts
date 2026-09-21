// ---------------------------------------------------------------------------
// Forge Ready gate — a story must not leave Planned (start a fresh FORGE_SDLC
// instance and burn Scout/Architect/Lead/Smith) when its packet cannot be
// assayed. FINAL-02 lesson: a FEATURE with no assay recipe used to HOLD forever
// only AFTER roles had already run. Fail closed at start instead: no acceptance
// criteria or no assay recipe on a QA-applicable story => keep it out of the
// run lane. An assay recipe that parses to zero commands counts as missing.
// Non-QA work types (RESEARCH, MIGRATION) are not gated here.
// ---------------------------------------------------------------------------

import { parseAssayCommands } from '@/agent-runtime/assay-plan'
import { acceptanceClauses } from '@/legacy/workflow_app/forge/agents/qa/types'

export const QA_APPLICABLE_WORK_TYPES: ReadonlySet<string> = new Set([
  'FEATURE',
  'BUG',
  'HOTFIX',
])

export type ReadyGateReason =
  | 'ready-gate:missing-acceptance'
  | 'ready-gate:missing-assay-plan'
  | 'ready-gate:unmapped-acceptance'

export type StoryReadyToRunFacts = {
  workType: string
  acceptanceCriteria: string | null | undefined
  assayCommands: string | null | undefined
  /**
   * The acceptance-to-assertion mapping declared before the work (clause -> assertion refs). When it IS
   * supplied, every acceptance clause must map to at least one assertion: a clause with no assertion can
   * only ever come back UNPROVEN, so the story is not ready to run until it is mapped.
   *
   * ABSENT is not gated here — that is the mapping's own absent case, which QA reports as UNPROVEN.
   */
  acceptanceAssertions?: Record<string, string[]> | null
}

/**
 * Pure predicate: reasons a QA-applicable story is NOT ready to start a run.
 * Empty when the story may leave Planned. Non-QA work types always return [].
 */
export function storyReadyToRunReasons(facts: StoryReadyToRunFacts): ReadyGateReason[] {
  if (!QA_APPLICABLE_WORK_TYPES.has(facts.workType)) return []
  const reasons: ReadyGateReason[] = []
  if (!facts.acceptanceCriteria?.trim()) reasons.push('ready-gate:missing-acceptance')
  if (parseAssayCommands(facts.assayCommands).length === 0)
    reasons.push('ready-gate:missing-assay-plan')
  if (facts.acceptanceAssertions) {
    const unmapped = acceptanceClauses(facts.acceptanceCriteria).filter(
      (clause) => (facts.acceptanceAssertions?.[clause] ?? []).length === 0,
    )
    if (unmapped.length > 0) reasons.push('ready-gate:unmapped-acceptance')
  }
  return reasons
}
