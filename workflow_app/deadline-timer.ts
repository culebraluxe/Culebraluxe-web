// ---------------------------------------------------------------------------
// Generic deadline-timer reconciliation (CRM-22).
//
// workflow_app projects canonical milestone dates into TIMER SEMANTICS:
// the engine persists/fires/reschedules/reclaims generic timer jobs
// (workflow_engine), and this seam keeps an instance's PENDING timer for a
// milestone node deterministically aligned with its canonical application
// date.
//
// When the canonical date changes while the instance is active, this seam
// RESCHEDULES the instance's existing pending timer for that milestone — the
// SAME workflow instance and the SAME timer job continue, never a duplicate
// and never a restart. No legal deadline is invented: if no canonical date
// exists (or no timer has been scheduled yet), nothing happens; the XML
// timer node schedules the job when the workflow reaches the milestone.
//
// The node id (the XML state identity) scopes the lookup: an instance may
// carry several pending timers (closing, inspection, financing), and each
// job's payload records the timer node that created it
// (payload.nodeId, written by the engine's _handleTimer).
//
// This module is generic: it names no workflow node and no jurisdiction; the
// node id is a parameter. `closing-timer.ts` is a thin wrapper over this
// seam for the pre-CRM-22 closing-date entry point.
// ---------------------------------------------------------------------------

export type DeadlineTimerDeps = {
  findPendingTimer: (
    instanceId: string,
    timerNodeId: string,
  ) => Promise<{ jobId: string; dueAt: string } | null>
  reschedule: (jobId: string, dueAt: Date) => Promise<void>
}

export type DeadlineTimerResult = {
  action: 'rescheduled' | 'unchanged'
  jobId: string | null
}

export async function reconcileDeadlineTimerCore(
  instanceId: string,
  timerNodeId: string,
  deadline: string | null,
  deps: DeadlineTimerDeps,
): Promise<DeadlineTimerResult> {
  const existing = await deps.findPendingTimer(instanceId, timerNodeId)

  // No canonical date, or no timer scheduled yet (the XML node schedules it
  // when the workflow reaches the milestone): nothing to reschedule.
  if (!deadline || !existing) {
    return { action: 'unchanged', jobId: existing?.jobId ?? null }
  }

  const target = new Date(deadline)
  if (existing.dueAt === deadline) {
    return { action: 'unchanged', jobId: existing.jobId }
  }

  await deps.reschedule(existing.jobId, target)
  return { action: 'rescheduled', jobId: existing.jobId }
}

export async function reconcileDeadlineTimer(
  instanceId: string,
  timerNodeId: string,
  deadline: string | null,
): Promise<DeadlineTimerResult> {
  const { engineConfigured, engineSql } = await import('./engine-client')
  if (!engineConfigured()) return { action: 'unchanged', jobId: null }

  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')
  const { createApplicationPort } = await import('./application-port')
  const esql = engineSql()
  const engine = () =>
    new WorkflowEngine(esql, { app: createApplicationPort() })

  return reconcileDeadlineTimerCore(instanceId, timerNodeId, deadline, {
    findPendingTimer: async (id, nodeId) => {
      const rows = await esql`
        select id, due_at::text as due_at
        from jobs
        where process_instance_id = ${id}
          and status = 'pending'
          and type = 'timer'
          and payload->>'nodeId' = ${nodeId}
        limit 1
      `
      const r = rows[0]
      return r
        ? { jobId: r.id as string, dueAt: r.due_at as string }
        : null
    },
    reschedule: async (jobId, dueAt) =>
      engine().rescheduleTimerJob({ jobId, newDueAt: dueAt, actor: 'system' }),
  })
}
