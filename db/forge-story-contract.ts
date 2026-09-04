import type { QueryExecutor } from './query-executor'
import type { ArchitectHandoff, ScoutHandoff } from '../agent-runtime/handoff-contract'

export async function applyForgeScoutHandoff(
  storyId: string,
  handoff: ScoutHandoff,
  q: QueryExecutor,
): Promise<void> {
  await q`
    update storyboard_story
    set context_refs = ${handoff.contextRefs},
        updated_at = now()
    where id = ${storyId}
  `
}

export async function applyForgeArchitectHandoff(
  storyId: string,
  handoff: ArchitectHandoff,
  q: QueryExecutor,
): Promise<void> {
  await q`
    update storyboard_story
    set architect_brief = ${handoff.architectBrief},
        acceptance_criteria = ${handoff.acceptanceCriteria},
        test_mode = ${handoff.testMode},
        assay_commands = ${handoff.assayCommands},
        architect_brief_updated_at = now(),
        updated_at = now()
    where id = ${storyId}
  `
}
