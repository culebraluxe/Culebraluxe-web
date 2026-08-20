// ---------------------------------------------------------------------------
// P&S closing-date timer reconciliation (Story 122 / 137).
//
// The RE_supermodel's `closing_date_timer` node (in the XML) owns scheduling
// the closing-deadline timer from the canonical `closingDate` fact. When that
// canonical date changes while the instance is active, this seam RESCHEDULES
// the instance's existing pending timer — the SAME workflow instance
// continues, never terminated/restarted. No legal deadline is invented: if no
// canonical closing date exists (or no timer has been scheduled yet), nothing
// happens; the node schedules it when the workflow reaches the closing stage.
//
// This module is generic: it names no workflow node and no jurisdiction.
// ---------------------------------------------------------------------------

export type ClosingTimerDeps = {
  findPendingTimer: (
    instanceId: string,
  ) => Promise<{ jobId: string; dueAt: string } | null>
  reschedule: (jobId: string, dueAt: Date) => Promise<void>
}

export type ClosingTimerResult = {
  action: 'rescheduled' | 'unchanged'
  jobId: string | null
}

export async function reconcileClosingTimerCore(
  instanceId: string,
  closingDate: string | null,
  deps: ClosingTimerDeps,
): Promise<ClosingTimerResult> {
  const existing = await deps.findPendingTimer(instanceId)

  // No canonical date, or no timer scheduled yet (the node schedules it when
  // the workflow reaches the closing stage): nothing to reschedule.
  if (!closingDate || !existing) {
    return { action: 'unchanged', jobId: existing?.jobId ?? null }
  }

  const target = new Date(closingDate)
  if (existing.dueAt === closingDate) {
    return { action: 'unchanged', jobId: existing.jobId }
  }

  await deps.reschedule(existing.jobId, target)
  return { action: 'rescheduled', jobId: existing.jobId }
}

export async function reconcileClosingTimer(
  instanceId: string,
  closingDate: string | null,
): Promise<ClosingTimerResult> {
  const { engineConfigured, engineSql } = await import('./engine-client')
  if (!engineConfigured()) return { action: 'unchanged', jobId: null }

  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')
  const { createApplicationPort } = await import('./application-port')
  const esql = engineSql()
  const engine = () =>
    new WorkflowEngine(esql, { app: createApplicationPort() })

  return reconcileClosingTimerCore(instanceId, closingDate, {
    findPendingTimer: async (id) => {
      const rows = await esql`
        select id, due_at::text as due_at
        from jobs
        where process_instance_id = ${id}
          and status = 'pending'
          and type = 'timer'
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
