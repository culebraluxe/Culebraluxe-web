import { PortalWriteError } from '../lib/portal-write-error'
import {
  getAgentWorkItem,
  listStaleAgentWork,
  type AgentWorkItem,
} from './agent-work'
import type { QueryExecutor } from './query-executor'

export type AgentWorkRecoveryDisposition = 'retry' | 'hold'

export type AgentWorkRecoveryResult = {
  workItem: AgentWorkItem
  disposition: AgentWorkRecoveryDisposition
  interruptedRunId: string | null
}

/**
 * Assay is a human intervention boundary.
 *
 * A verifier/reviewer interruption must NEVER consume retry budget by
 * automatically starting another Assay, and must never route back to Smith.
 * The candidate + failed/interrupted evidence are preserved on Hold until a
 * human deliberately chooses the next action.
 */
export function assayInterruptionRequiresHuman(
  role: string | null | undefined,
): boolean {
  const normalized = (role ?? '').trim().toLowerCase()
  return normalized === 'verifier' || normalized === 'reviewer'
}

/**
 * Runtime interruption is not story failure.
 *
 * This is the durable recovery seam for a worker process that died, exited
 * non-zero, or was orphaned by a host restart. The interrupted run is closed
 * truthfully as `Interrupted`; ordinary execution work is then either re-queued
 * for another process attempt or held after its retry budget is exhausted.
 *
 * ASSAY EXCEPTION: verifier/reviewer is an explicit human intervention point.
 * Any Assay interruption goes directly to Hold on the first interruption,
 * regardless of remaining retry budget. It is never automatically re-run and
 * never causes Smith to restart.
 *
 * Important: story_run_id is intentionally preserved on an ordinary re-queued
 * row until the next beginRun() replaces it. The caller that observed the
 * process failure can therefore still normalize and return evidence for the
 * exact interrupted run after this transition.
 */
export async function recoverAgentWorkInterruption(
  workItemId: string,
  reason: string,
  execute: QueryExecutor,
): Promise<AgentWorkRecoveryResult> {
  const item = await getAgentWorkItem(workItemId, execute)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }

  if (item.state === 'Done' || item.state === 'Error' || item.state === 'Cancelled') {
    return {
      workItem: item,
      disposition: 'hold',
      interruptedRunId: item.storyRunId,
    }
  }

  const conciseReason = String(reason || 'runtime interrupted').slice(0, 2000)
  const interruptedRunId = item.storyRunId
  const humanGate = assayInterruptionRequiresHuman(item.role)
  const exhausted = humanGate || item.attempts >= item.maxAttempts

  if (exhausted) {
    const errorText = humanGate
      ? `Assay interrupted; human intervention required: ${conciseReason}`
      : `runtime retry budget exhausted (${item.attempts}/${item.maxAttempts}): ${conciseReason}`
    const runNote = humanGate
      ? `Assay interrupted: ${conciseReason}. Human intervention required; no automatic Assay retry and no Smith restart.`
      : `runtime interrupted: ${conciseReason}`

    // One statement = one atomic recovery decision. The run closes as
    // Interrupted, the work item becomes terminal, and the story moves to Hold
    // without ever claiming implementation failure. For Assay this happens on
    // the FIRST interruption even when retry budget remains.
    await execute`
      with interrupted_run as (
        update storyboard_story_run
        set ended_at = now(),
            result_status = 'Interrupted',
            notes = case
              when notes is null or notes = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${runNote}
              else notes || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${runNote}
            end,
            updated_at = now()
        where id = ${interruptedRunId}
          and ended_at is null
        returning id
      ), exhausted_work as (
        update agent_work_item
        set state = 'Error',
            error_text = ${errorText},
            finished_at = now(),
            updated_at = now()
        where id = ${workItemId}
          and state in ('Claimed', 'Running', 'Paused')
        returning story_id
      )
      update storyboard_story s
      set status = 'Hold',
          completed_at = null,
          updated_at = now()
      from exhausted_work w
      where s.id = w.story_id
    `
  } else {
    // Row FIRST inside the same statement, then story -> Ready. This matters
    // for Paused: the historical per-story active index excludes Paused, so a
    // story-first Ready transition could let the dispatch trigger create a
    // duplicate queue row. By the time that trigger fires here, THIS work item
    // is already Ready and its insert conflicts safely with the existing row.
    // This automatic retry path is deliberately unreachable for Assay roles.
    await execute`
      with interrupted_run as (
        update storyboard_story_run
        set ended_at = now(),
            result_status = 'Interrupted',
            notes = case
              when notes is null or notes = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
              else notes || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
            end,
            updated_at = now()
        where id = ${interruptedRunId}
          and ended_at is null
        returning id
      ), released_work as (
        update agent_work_item
        set state = 'Ready',
            queued_at = now(),
            claimed_at = null,
            claimed_by = null,
            started_at = null,
            finished_at = null,
            error_text = null,
            runtime_adapter = null,
            external_run_id = null,
            updated_at = now()
        where id = ${workItemId}
          and state in ('Claimed', 'Running', 'Paused')
        returning story_id
      )
      update storyboard_story s
      set status = 'Ready',
          completed_at = null,
          updated_at = now()
      from released_work w
      where s.id = w.story_id
    `
  }

  const recovered = await getAgentWorkItem(workItemId, execute)
  if (!recovered) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" disappeared during recovery.`)
  }
  return {
    workItem: recovered,
    disposition: exhausted ? 'hold' : 'retry',
    interruptedRunId,
  }
}

/**
 * Host/scheduler recovery uses the exact same primitive as an observed child
 * process failure. No special stale-work semantics, no conversion to story
 * Failed, and no destruction of the worker branch/worktree. Because the same
 * primitive is used, a stale Assay also stops on Hold for human intervention.
 */
export async function recoverStaleAgentWorkIndustrial(
  staleAfterMinutes: number,
  execute: QueryExecutor,
): Promise<AgentWorkRecoveryResult[]> {
  const stale = await listStaleAgentWork(staleAfterMinutes, execute)
  const recovered: AgentWorkRecoveryResult[] = []
  for (const item of stale) {
    recovered.push(
      await recoverAgentWorkInterruption(
        item.id,
        `stale worker: no heartbeat since ${item.updatedAt}; process/host presumed terminated`,
        execute,
      ),
    )
  }
  return recovered
}
