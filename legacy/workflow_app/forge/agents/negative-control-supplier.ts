/**
 * THE STORY'S DECLARED NEGATIVE CONTROL, SUPPLIED TO THE PRODUCTION PORTS
 * (FORGE-NEGATIVE-CONTROL-WIRING-01).
 *
 * WHY THIS IS A MODULE AND NOT THREE INLINE LINES. Astra review 1.4 measured the defect precisely: the
 * collector and the adjudicator both support a negative control, the test supplied one by hand, and the
 * production `rolePorts` never did — so no ordinary run could reach the capability. A supplier that only
 * exists inline in the runner cannot be fenced, which is how it stayed unsupplied. Here it is a pure
 * function over the resolved story, so the rule is testable without a lane.
 *
 * THE RULE, and its honest edge: a story whose `negativeControlCommand` is absent supplies NOTHING
 * (`undefined`, never `{}`), which keeps the collector's semantics exactly as they were — an absent control
 * is not a pass, and the evidence records no control rather than a vacuous one. A blank or whitespace-only
 * value is treated as absent for the same reason: a command that is "nothing" cannot be run, and pretending
 * to run it would be worse than saying we did not.
 */
import type { NegativeControl } from '@/legacy/workflow_app/forge/agents/qa/types'

export type StoryNegativeControlSource = {
  /** The command that inverts this story's fence and runs it again. Null/absent = no control declared. */
  negativeControlCommand?: string | null
}

export function negativeControlForStory(
  story: StoryNegativeControlSource | null | undefined,
): NegativeControl | undefined {
  const command = (story?.negativeControlCommand ?? '').trim()
  if (!command) return undefined
  // `assertions` is deliberately omitted: the plan's acceptance mapping already names what the fence
  // proves, and the adjudicator reads an absent list as "every mapped assertion is intended".
  return { command }
}
