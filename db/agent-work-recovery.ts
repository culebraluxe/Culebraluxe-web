import { PortalWriteError } from '../lib/portal-write-error'
import { decideRuntimeRecovery } from '../agent-runtime/recovery-policy'
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

/** Backward-compatible predicate used by focused tests/operator tooling. */
export function assayInterruptionRequiresHuman(
  role: string | null | undefined,
): boolean {
  return (
    decideRuntimeRecovery({
      role,
      attempts: 0,
      maxAttempts: 3,
      reason: 'policy probe',
    }).action === 'hold-human'
  )
}

function interruptionFailureCode(role: string | null | undefined): string {
  const normalized = (role ?? '').trim().toLowerCase()
  if (normalized === 'reviewer' || normalized === 'verifier') {
    return 'ASSAY_RUNTIME_INTERRUPTED'
  }
  if (normalized === 'builder') return 'SMITH_RUNTIME_INTERRUPTED'
  return 'HUMAN_DECISION_REQUIRED'
}

/**
 * Runtime recovery is policy, not prose.
 *
 * Smith/ordinary infrastructure interruptions may retry while budget remains.
 * Assay interruption is always a human intervention point: Hold immediately,
 * no automatic Assay retry and no Smith restart.
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
  const recovery = decideRuntimeRecovery({
    role: item.role,
    attempts: item.attempts,
    maxAttempts: item.maxAttempts,
    reason: conciseReason,
  })
  const hold = recovery.action === 'hold-human'
  const failureCode = interruptionFailureCode(item.role)
  const recoveryDetail =
    `recovery action=${recovery.action} human_required=${recovery.humanRequired} ` +
    `attempts=${item.attempts}/${item.maxAttempts} reason=${recovery.reason}`

  if (hold) {
    await execute`
      with interrupted_run as (
        update storyboard_story_run
        set ended_at = now(),
            result_status = 'Interrupted',
            failure_code = ${failureCode},
            notes = case
              when notes is null or notes = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${recovery.reason}
              else notes || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${recovery.reason}
            end,
            evidence_detail = case
              when evidence_detail is null or evidence_detail = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${recoveryDetail}
              else evidence_detail || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${recoveryDetail}
            end,
            updated_at = now()
        where id = ${interruptedRunId}
          and ended_at is null
        returning id
      ), held_work as (
        update agent_work_item
        set state = 'Error',
            error_text = ${recovery.reason},
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
      from held_work w
      where s.id = w.story_id
    `
  } else {
    // Release the existing work row before Story -> Ready so the dispatch
    // trigger cannot create a duplicate active item. This path is unreachable
    // for Assay roles by policy.
    await execute`
      with interrupted_run as (
        update storyboard_story_run
        set ended_at = now(),
            result_status = 'Interrupted',
            failure_code = ${failureCode},
            notes = case
              when notes is null or notes = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
              else notes || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — runtime interrupted: ' || ${conciseReason}
            end,
            evidence_detail = case
              when evidence_detail is null or evidence_detail = ''
                then to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${recoveryDetail}
              else evidence_detail || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${recoveryDetail}
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
    disposition: hold ? 'hold' : 'retry',
    interruptedRunId,
  }
}

/** Host/scheduler recovery uses the exact same policy primitive. */
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
