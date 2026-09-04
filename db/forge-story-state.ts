import type { QueryExecutor } from './query-executor'

/**
 * V6 lane completion is not story completion. Smith and Assay may each finish
 * successfully while the accepted candidate is still awaiting the next gate.
 */
export async function markForgeStoryInProgress(
  storyId: string,
  execute: QueryExecutor,
): Promise<void> {
  await execute`
    update storyboard_story
    set status = 'In Progress',
        completed_at = null,
        updated_at = now()
    where id = ${storyId}
  `
}

export async function markForgeStoryHumanHold(
  storyId: string,
  execute: QueryExecutor,
): Promise<void> {
  await execute`
    update storyboard_story
    set status = 'Hold',
        completed_at = null,
        updated_at = now()
    where id = ${storyId}
  `
}

/** Story Complete is owned by successful outer publication in Forge V6. */
export async function markForgeStoryPublishedComplete(
  storyId: string,
  execute: QueryExecutor,
): Promise<void> {
  await execute`
    update storyboard_story
    set status = 'Complete',
        completion = 100,
        completed_at = coalesce(completed_at, now()),
        updated_at = now()
    where id = ${storyId}
  `
}
