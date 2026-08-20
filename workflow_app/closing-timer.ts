// ---------------------------------------------------------------------------
// P&S closing-date timer reconciliation.
//
// deal.closing_date is the canonical current P&S target closing date. When it
// changes while transaction-close-v1 is active:
//   - the SAME workflow instance continues (never terminated/restarted),
//   - the closing-deadline timer is rescheduled idempotently,
//   - the obsolete due_at is replaced (the old timer cannot later fire as
//     though still authoritative).
//
// No legal deadline is invented: if no canonical closing date exists, nothing
// is scheduled.
// ---------------------------------------------------------------------------

export type ClosingTimerDeps = {
  findPendingTimer: (
    instanceId: string,
  ) => Promise<{ jobId: string; dueAt: string } | null>
  schedule: (instanceId: string, dueAt: Date) => Promise<string>
  reschedule: (jobId: string, dueAt: Date) => Promise<void>
}

export type ClosingTimerResult = {
  action: 'scheduled' | 'rescheduled' | 'unchanged'
  jobId: string | null
}

export async function reconcileClosingTimerCore(
  instanceId: string,
  closingDate: string | null,
  deps: ClosingTimerDeps,
): Promise<ClosingTimerResult> {
  const existing = await deps.findPendingTimer(instanceId)

  if (!closingDate) {
    // No canonical date: leave any existing timer untouched, never invent one.
    return { action: 'unchanged', jobId: existing?.jobId ?? null }
  }

  const target = new Date(closingDate)
  if (!existing) {
    const jobId = await deps.schedule(instanceId, target)
    return { action: 'scheduled', jobId }
  }

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
          and payload->>'kind' = 'closing_deadline'
        limit 1
      `
      const r = rows[0]
      return r
        ? { jobId: r.id as string, dueAt: r.due_at as string }
        : null
    },
    schedule: async (id, dueAt) =>
      engine().createJob({
        processInstanceId: id,
        type: 'timer',
        dueAt,
        payload: { kind: 'closing_deadline' },
        maxAttempts: 5,
      }),
    reschedule: async (jobId, dueAt) =>
      engine().rescheduleTimerJob({ jobId, newDueAt: dueAt, actor: 'system' }),
  })
}
