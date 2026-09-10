// ---------------------------------------------------------------------------
// ENG-FORGE-SPLIT-01 — durable per-child facts for a SPLIT fan-out.
//
// The join gate needs to know, for every expected child: did it reach a terminal
// state, and which candidate SHA did IT produce (never a sibling's, never the
// parent checkout's — capturing the wrong workspace's candidate is the incident
// that locked the SPLIT door). These columns already existed on agent_work_item
// (split_assignment, parallel_slot, candidate_shas) and were never written;
// this module is the writer/reader so no new table is invented.
//
// Pure SQL, repository-owned normalization: driver values leave this module as
// plain strings/numbers/arrays (never driver-specific objects).
// ---------------------------------------------------------------------------

import type { QueryExecutor } from './query-executor'
import type { SplitOutcome } from '../workflow_app/forge/split-join'

async function executor(): Promise<QueryExecutor> {
  const client = await import('./client')
  return client.sql
}

/** Record which accepted Lead assignment this child owns. */
export async function recordSplitChildAssignment(
  workItemId: string,
  input: { assignmentId: string; index: number },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update agent_work_item
    set split_assignment = ${input.assignmentId},
        parallel_slot = ${input.index},
        updated_at = now()
    where id = ${workItemId}
  `
}

/**
 * Record the candidate SHA THIS child produced from ITS OWN workspace.
 * Written from the child's own run evidence, so a sibling's SHA can never be
 * mistaken for this child's output.
 */
export async function recordSplitChildCandidate(
  workItemId: string,
  candidateSha: string,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const sha = candidateSha.trim()
  if (!sha) return
  await q`
    update agent_work_item
    set candidate_shas = array_append(coalesce(candidate_shas, '{}'::text[]), ${sha}),
        updated_at = now()
    where id = ${workItemId}
  `
}

/**
 * Terminal outcomes for a story's split children, as the join reducer's input.
 *
 * Only TERMINAL children are returned: a child still Ready/Claimed/Running is
 * deliberately absent so `reduceSplit` reports it as MISSING — "not finished" is
 * not "finished with no evidence".
 */
export async function listSplitChildOutcomes(
  storyId: string,
  execute?: QueryExecutor,
): Promise<{ outcomes: SplitOutcome[]; unrecorded: string[] }> {
  const q = execute ?? (await executor())
  const rows = await q`
    select split_assignment, parallel_slot, state, attempts, candidate_shas
    from agent_work_item
    where story_id = ${storyId}
      and split_assignment is not null
    order by parallel_slot nulls last, created_at
  `
  const outcomes: SplitOutcome[] = []
  const unrecorded: string[] = []
  for (const row of rows as Array<{
    split_assignment: string
    parallel_slot: number | null
    state: string
    attempts: number | null
    candidate_shas: string[] | null
  }>) {
    const childId = String(row.split_assignment)
    const shas = (row.candidate_shas ?? []).map((s) => String(s))
    const common = {
      childId,
      attempt: Math.max(1, Number(row.attempts ?? 1)),
      candidateSha: shas.length ? shas[shas.length - 1] : null,
    }
    switch (row.state) {
      case 'Done':
        if (!common.candidateSha) unrecorded.push(childId)
        outcomes.push({ ...common, status: 'completed' })
        break
      case 'Failed':
        outcomes.push({ ...common, status: 'failed' })
        break
      case 'Cancelled':
      case 'Paused':
        outcomes.push({ ...common, status: 'cancelled' })
        break
      default:
        // Non-terminal: omitted on purpose (reduceSplit reports it as missing).
        break
    }
  }
  return { outcomes, unrecorded }
}
