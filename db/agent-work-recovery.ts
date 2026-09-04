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
 * Runtime interruption is not story failure.
 *
 * This is the durable recovery seam for a worker process that died, exited
 * non-zero, or was orphaned by a host restart. The interrupted run is closed
 * truthfully as `Interrupted`; the SAME work item is then either re-queued for
 * another process attempt or held after its retry budget is exhausted.
 *
 * Important: story_run_id is intentionally preserved on the re-queued row
 * until the next beginRun() replaces it. The caller that observed the process
 * failure can therefore still normalize and return evidence for the exact
 * interrupted run after this transition.
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

  // Close only an ACTIVE run. Recovery is idempotent: a second pass never
  // rewrites an already-terminal run or appends duplicate interruption notes.
  if (interruptedRunId) {
    await execute`
      update storyboard_story_run
      set ended_at = now(),
          result_status = 'Interrupted',
          notes = case
            when notes is null or notes = ''
              then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
            else notes || E'\\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
          end,
          updated_at = now()
      where id = ${interruptedRunId}
        and ended_at is null
    `
  }

  const exhausted = item.attempts >= item.maxAttempts
  if (exhausted) {
    // Retry budget is infrastructure/process budget, not acceptance evidence.
    // Preserve the interrupted run/worktree and Hold for a human decision.
    await execute`
      update storyboard_story
      set status = 'Hold',
          completed_at = null,
          updated_at = now()
      where id = ${item.storyId}
    `
    await execute`
      update agent_work_item
      set state = 'Error',
          error_text = ${`runtime retry budget exhausted (${item.attempts}/${item.maxAttempts}): ${conciseReason}`},
          finished_at = now(),
          updated_at = now()
      where id = ${workItemId}
        and state in ('Claimed', 'Running', 'Paused')
    `
  } else {
    // Set the story Ready while this row is still active. The existing Ready
    // dispatch trigger sees the active row and therefore cannot create a
    // duplicate queue item. Then release this SAME item back to Ready.
    await execute`
      update storyboard_story
      set status = 'Ready',
          completed_at = null,
          updated_at = now()
      where id = ${item.storyId}
    `
    await execute`
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
 * Failed, and no destruction of the worker branch/worktree.
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
