// ---------------------------------------------------------------------------
// Forge Ready gate — a story must not leave Planned (start a fresh FORGE_SDLC
// instance and burn Scout/Architect/Lead/Smith) when its packet cannot be
// assayed. FINAL-02 lesson: a FEATURE with no assay recipe used to HOLD forever
// only AFTER roles had already run. Fail closed at start instead: no acceptance
// criteria or no assay recipe on a QA-applicable story => keep it out of the
// run lane. An assay recipe that parses to zero commands counts as missing.
// Non-QA work types (RESEARCH, MIGRATION) are not gated here.
// ---------------------------------------------------------------------------

import { parseAssayCommands } from '../../agent-runtime/assay-plan'

export const QA_APPLICABLE_WORK_TYPES: ReadonlySet<string> = new Set([
  'FEATURE',
  'BUG',
  'HOTFIX',
])

export type ReadyGateReason =
  | 'ready-gate:missing-acceptance'
  | 'ready-gate:missing-assay-plan'

export type StoryReadyToRunFacts = {
  workType: string
  acceptanceCriteria: string | null | undefined
  assayCommands: string | null | undefined
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
  return reasons
}
