import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** V6 lane completion is not story completion. */
export async function markForgeStoryInProgress(
  storyId: string,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set status = 'In Progress',
        completed_at = null,
        updated_at = now()
    where id = ${storyId}
  `
}

export async function markForgeStoryHumanHold(
  storyId: string,
  reason?: string | null,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set status = 'Hold',
        completed_at = null,
        notes = case
          when ${reason?.trim() || null}::text is null then notes
          when notes is null or notes = '' then ${reason?.trim() || null}
          else notes || E'\n' || ${reason?.trim() || null}
        end,
        updated_at = now()
    where id = ${storyId}
  `
}

export async function markForgeStoryFailed(
  storyId: string,
  reason?: string | null,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set status = 'Failed',
        completed_at = null,
        notes = case
          when ${reason?.trim() || null}::text is null then notes
          when notes is null or notes = '' then ${reason?.trim() || null}
          else notes || E'\n' || ${reason?.trim() || null}
        end,
        updated_at = now()
    where id = ${storyId}
  `
}

/** Story Complete is owned by successful outer publication in Forge V6. */
export async function markForgeStoryPublishedComplete(
  storyId: string,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set status = 'Complete',
        completion = 100,
        completed_at = coalesce(completed_at, now()),
        updated_at = now()
    where id = ${storyId}
  `
}
