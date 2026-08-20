// ---------------------------------------------------------------------------
// V1 workflow reconciliation entrypoint.
//
// One idempotent application-level pass that:
//   1. starts transaction-close-v1 for accepted-offer deals missing an instance,
//   2. materializes missing operational tasks,
//   3. reconciles canonical closing-date timers.
//
// Suitable later for a Vercel Cron, an admin action, or an operational worker.
// Not a scheduler framework; no cron is configured here.
// ---------------------------------------------------------------------------

export type ReconcileReport = {
  startedInstances: number
  materializedTasks: number
  skippedTasks: number
}

export type ReconcileSteps = {
  startMissing: () => Promise<string[]>
  materializeTasks: () => Promise<{ materialized: number; skipped: number }>
}

export async function reconcileWorkflowsCore(
  steps: ReconcileSteps,
): Promise<ReconcileReport> {
  const [started, materialized] = await Promise.all([
    steps.startMissing(),
    steps.materializeTasks(),
  ])
  return {
    startedInstances: started.length,
    materializedTasks: materialized.materialized,
    skippedTasks: materialized.skipped,
  }
}

export async function reconcileWorkflows(): Promise<ReconcileReport> {
  const { reconcileTransactionWorkflows } = await import('./runtime')
  const { reconcileTaskMaterialization } = await import('./task-reconciliation')
  return reconcileWorkflowsCore({
    startMissing: reconcileTransactionWorkflows,
    materializeTasks: reconcileTaskMaterialization,
  })
}
