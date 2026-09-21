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

import type { QueryExecutor } from '@/legacy/db/query-executor'
import type { SplitOutcome } from '@/legacy/workflow_app/forge/split-join'

async function executor(): Promise<QueryExecutor> {
  const client = await import('@/legacy/db/client')
  return client.sql
}

/**
 * Record which accepted Lead assignment this child owns.
 *
 * `index` is the engine's 0-based branch index; the persisted slot is 1-BASED
 * (`index + 1`) to match the enqueue, which allocates `parallel_slot = index + 1`.
 * Two writers using different conventions for the same column produced a duplicate
 * (story, group, slot) and a 23505 on a live run — keep them in step.
 *
 * FORGE-PARITY-CHECK-01 — the group is written HERE too, and that is the fix, not
 * cosmetics. This UPDATE used to set `split_assignment` and `parallel_slot` only, so
 * the row's shape depended on whether the row it updated already carried a group.
 * A row without one came out as `parallel_group_id IS NULL` with a slot and an
 * assignment set — precisely the shape PROD's
 * `agent_work_item_parallel_shape_check` refuses and DEV (which had no such
 * constraint) accepted. Two such rows were sitting in DEV as the fingerprint of
 * this path. The enqueue and this writer now set the SAME tuple from the same
 * source, so a child row can never come out half-grouped.
 */
export async function recordSplitChildAssignment(
  workItemId: string,
  input: { assignmentId: string; index: number; groupId: string },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const assignmentId = input.assignmentId?.trim()
  const groupId = input.groupId?.trim()
  // Fail by NAME rather than writing a row the other environment rejects: a
  // half-grouped child is worse than a loud refusal, because on PROD it becomes a
  // 23514 deep inside a run and on DEV it becomes silent drift.
  if (!assignmentId) {
    throw new Error('recordSplitChildAssignment: assignmentId is required for a split child')
  }
  if (!groupId) {
    throw new Error(
      'recordSplitChildAssignment: groupId is required — a slot without a parallel group is a ' +
        'shape agent_work_item_parallel_shape_check refuses',
    )
  }
  if (!Number.isInteger(input.index) || input.index < 0) {
    throw new Error(
      `recordSplitChildAssignment: index must be a 0-based integer (got ${String(input.index)}) — ` +
        'the stored slot is index + 1, so a negative index would write slot <= 0',
    )
  }
  await q`
    update agent_work_item
    set split_assignment = ${assignmentId},
        parallel_group_id = ${groupId},
        parallel_slot = ${input.index + 1},
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
 * MUST be scoped to ONE fan-out: a story accumulates child rows across runs (and
 * template/rehearsal rows), and the join must never reduce a sibling from a
 * different attempt. `groupId` is the run's parallel group (the process instance).
 *
 * Only TERMINAL children are returned: a child still Ready/Claimed/Running is
 * deliberately absent so `reduceSplit` reports it as MISSING — "not finished" is
 * not "finished with no evidence".
 */
export async function listSplitChildOutcomes(
  storyId: string,
  options: { groupId?: string | null } = {},
  execute?: QueryExecutor,
): Promise<{ outcomes: SplitOutcome[]; unrecorded: string[] }> {
  const q = execute ?? (await executor())
  const rows = options.groupId
    ? await q`
        select split_assignment, parallel_slot, state, attempts, candidate_shas
        from agent_work_item
        where story_id = ${storyId}
          and split_assignment is not null
          and parallel_group_id = ${options.groupId}
        order by parallel_slot nulls last, created_at
      `
    : await q`
        select split_assignment, parallel_slot, state, attempts, candidate_shas
        from agent_work_item
        where story_id = ${storyId}
          and split_assignment is not null
        order by parallel_slot nulls last, created_at
      `
  const byChild = new Map<string, SplitOutcome>()
  for (const row of rows as Array<{
    split_assignment: string
    parallel_slot: number | null
    state: string
    attempts: number | null
    candidate_shas: string[] | null
  }>) {
    const childId = String(row.split_assignment)
    const shas = (row.candidate_shas ?? []).map((s) => String(s))
    const status: SplitOutcome['status'] | null =
      row.state === 'Done'
        ? 'completed'
        : row.state === 'Failed'
          ? 'failed'
          : row.state === 'Cancelled' || row.state === 'Paused'
            ? 'cancelled'
            : null
    // Non-terminal (Ready/Claimed/Running): omitted on purpose so the reducer
    // reports it as MISSING — "not finished" is not "finished with no evidence".
    if (status === null) continue
    const outcome: SplitOutcome = {
      childId,
      status,
      attempt: Math.max(1, Number(row.attempts ?? 1)),
      candidateSha: shas.length ? shas[shas.length - 1] : null,
    }
    // One child row per slot per group, but keep the most advanced attempt.
    const existing = byChild.get(childId)
    if (!existing || outcome.attempt >= existing.attempt) byChild.set(childId, outcome)
  }
  // The unrecorded check is per CHILD, not per ROW: a story (and even one fan-out) can
  // hold more than one row for the same child when an attempt is superseded, and the
  // reducer already collapses those. Counting a superseded row as "no SHA" would HOLD
  // a join whose children all produced candidates.
  const unrecorded = [...byChild.values()]
    .filter((o) => o.status === 'completed' && !o.candidateSha)
    .map((o) => o.childId)
  return { outcomes: [...byChild.values()], unrecorded }
}
