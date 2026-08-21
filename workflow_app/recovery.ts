// ---------------------------------------------------------------------------
// Workflow operational recovery / repair seam (CRM-14H).
//
// Narrow, generic recovery surfaces on top of the engine's lease primitives
// (CRM-14F). Persistent PostgreSQL state is the ONLY source of truth for
// recovery: the engine holds no correctness-affecting in-process state, so a
// fresh engine can always advance runnable work from the DB alone.
//
// Authority: the ENGINE stays authority-free. These seams are invoked through
// the application boundary; an operator authority check (AUTH-03 seam) is the
// caller's responsibility before invoking any repair.
//
// SAFE AUTO-REPAIR vs HUMAN-REQUIRED (explicit, not ad-hoc):
//   auto:   reclaim stale job leases (idempotent, side-effect-free)
//   auto:   re-run reconcile (start-missing instances + materialize tasks)
//   human:  poisoned command receipts, failed/error instance disposition,
//           any destructive reset
// ---------------------------------------------------------------------------
import { engineConfigured, engineSql } from './engine-client'
import { reconcileWorkflows } from './reconcile'
import type { ReconcileReport } from './reconcile'

export type RecoveryReport = {
  reclaimedStaleJobs: number
  reconcile: ReconcileReport
  anomalies: Array<{ kind: string; severity: string; instanceId: string | null; message: string }>
}

export type RecoveryDeps = {
  reclaimStaleJobs: (batch?: number) => Promise<number>
  reconcile: () => Promise<ReconcileReport>
  collectAnomalies: () => Promise<
    Array<{ kind: string; severity: string; instanceId: string | null; message: string }>
  >
}

/**
 * One normal, idempotent startup/on-demand recovery pass:
 * reclaim stale leases + re-run reconcile + surface anomalies.
 * Safe to run any number of times (all steps are idempotent).
 */
export async function runRecoveryPassCore(deps: RecoveryDeps): Promise<RecoveryReport> {
  const [reclaimedStaleJobs, reconcile, anomalies] = await Promise.all([
    deps.reclaimStaleJobs(50),
    deps.reconcile(),
    deps.collectAnomalies(),
  ])
  return { reclaimedStaleJobs, reconcile, anomalies }
}

export async function runRecoveryPass(): Promise<RecoveryReport> {
  if (!engineConfigured()) {
    return { reclaimedStaleJobs: 0, reconcile: { startedInstances: 0, materializedTasks: 0, skippedTasks: 0 }, anomalies: [] }
  }

  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')
  const { createApplicationPort } = await import('./application-port')
  const esql = engineSql()
  const engine = () => new WorkflowEngine(esql, { app: createApplicationPort() })

  const { getWorkflowDiagnosticsSnapshot } = await import('./diagnostics')

  return runRecoveryPassCore({
    reclaimStaleJobs: async (batch) => engine().reclaimStaleJobs(batch ?? 50),
    reconcile: reconcileWorkflows,
    collectAnomalies: async () => {
      const snap = await getWorkflowDiagnosticsSnapshot()
      return snap.anomalies
    },
  })
}

export type InstanceReconcileReport = {
  instanceId: string
  reclaimedStaleJobs: number
}

/**
 * Narrow per-instance reconciliation (operator repair scope):
 * only the targeted instance's stale job leases are reclaimed. No broad
 * table rewrites, no invented business transitions.
 */
export async function reconcileInstanceCore(
  instanceId: string,
  reclaimForInstance: (id: string) => Promise<number>,
): Promise<InstanceReconcileReport> {
  const reclaimedStaleJobs = await reclaimForInstance(instanceId)
  return { instanceId, reclaimedStaleJobs }
}

export async function reconcileInstance(instanceId: string): Promise<InstanceReconcileReport> {
  if (!engineConfigured()) {
    return { instanceId, reclaimedStaleJobs: 0 }
  }
  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')
  const { createApplicationPort } = await import('./application-port')
  const esql = engineSql()
  const engine = () => new WorkflowEngine(esql, { app: createApplicationPort() })

  return reconcileInstanceCore(instanceId, (id) => engine().reclaimStaleJobsForInstance(id))
}
