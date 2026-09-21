// ---------------------------------------------------------------------------
// FORGE LEARN — the data half of the learn loop (ENG-FORGE-FACTORY-01 Phase 3).
//
// Three small operations, each doing exactly one thing: what is already open, stage a learn member, and
// open a Ready learn item. The DECISION about which pattern to file lives in `lib/forge-learn.ts` (pure)
// and the orchestration in `agent-runtime/learn-loop.ts`, so nothing here chooses anything.
//
// The two extra facts a learn member carries over an ordinary staged story are `kind = 'learn'` (Phase 1's
// router) and `learn_pattern_key` (migration 181). The key is what makes the "never twice for the same
// pattern" rule enforceable by the database rather than by a check-then-insert.
// ---------------------------------------------------------------------------

import { ENGINE_DISPATCH_STATUS } from '@/lib/story-moves'
import { ensureStagingBatch } from '@/legacy/db/forge-batch'
import { setStoryboardStatus } from '@/legacy/db/storyboard'
import type { QueryExecutor } from '@/legacy/db/query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('@/legacy/db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * Pattern keys that already have an OPEN item: a queued/claimed/running work item, or a staged batch
 * member. Both count, because a staged member is work waiting to fire and filing it again would duplicate
 * the same learning twice.
 */
export async function listOpenLearnPatternKeys(execute?: QueryExecutor): Promise<string[]> {
  const q = execute ?? (await executor())
  const rows = (await q`
    select learn_pattern_key as key from agent_work_item
    where learn_pattern_key is not null and state in ('Ready', 'Claimed', 'Running', 'Paused')
    union
    select learn_pattern_key as key from forge_batch_item
    where learn_pattern_key is not null and state = 'Staged'
  `) as Array<{ key: string }>
  return rows.map((row) => String(row.key))
}

/**
 * Stage a learn story in the open staging batch.
 *
 * Deliberately not `stageStoryForBatch`: a learn member carries `kind` and `learn_pattern_key`, and the
 * generic stage has no business knowing about either. Staging writes the row the run will read (the
 * Autosys model), so a learn item queued at night fires with everything else.
 */
export async function stageLearnStory(
  input: { storyId: string; patternKey: string },
  execute?: QueryExecutor,
): Promise<{ batchId: string; staged: boolean }> {
  const q = execute ?? (await executor())
  const batch = await ensureStagingBatch(null, q)
  const rows = await q`
    insert into forge_batch_item (batch_id, story_id, state, kind, learn_pattern_key)
    values (${batch.id}, ${input.storyId}, 'Staged', 'learn', ${input.patternKey})
    on conflict (batch_id, story_id) do nothing
    returning story_id
  `
  return { batchId: batch.id, staged: rows.length > 0 }
}

/**
 * Open a READY learn item — the P0 path, for a pattern that should not wait for the batch.
 *
 * The dispatch is the engine's own trigger (`status Ready`), exactly like every other handover, and then
 * the routing facts are stamped on the item the trigger just created. Returns the number of rows stamped,
 * so a caller can tell "opened" from "there was nothing to stamp" (the same discipline as
 * `setWorkItemRouting`).
 */
export async function openReadyLearnItem(
  input: { storyId: string; patternKey: string; instructions: string },
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  await setStoryboardStatus(input.storyId, ENGINE_DISPATCH_STATUS, q)
  const rows = await q`
    update agent_work_item
    set kind = 'learn', learn_pattern_key = ${input.patternKey},
        special_instructions = ${input.instructions}, updated_at = now()
    where story_id = ${input.storyId} and state = 'Ready'
    returning id
  `
  return rows.length
}
