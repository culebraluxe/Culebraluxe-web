import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// Terminal invariant anomaly detectors (TUNIT harvest mechanism #15).
//
// Pure, dependency-injected core: every detector reads its input rows through
// the injected `execute` executor, so the sweep is durably testable with an
// in-memory fake and runs against the live database through the same code
// path (workflow_app/diagnostics.ts wires in the real `sql` handle).
//
// READ-ONLY. No mutation is performed here or by the live sweep.
// ---------------------------------------------------------------------------

export type WorkflowAnomaly = {
  kind: string
  severity: 'info' | 'warning' | 'critical'
  instanceId: string | null
  subjectId: string | null
  message: string
}

/** The instance fields the anomaly detectors need. */
export type AnomalyInstanceInput = {
  instanceId: string
  definitionKey: string
  definitionVersion: number
  subjectId: string | null
  status: string
  outcome: string | null
}

export async function collectAnomalies(
  anomalies: WorkflowAnomaly[],
  instances: AnomalyInstanceInput[],
  execute: QueryExecutor,
): Promise<void> {
  // 1. Failed/error processes.
  for (const i of instances) {
    if (i.status === 'error' || i.outcome === 'failed') {
      anomalies.push({
        kind: 'failed-process',
        severity: 'critical',
        instanceId: i.instanceId,
        subjectId: i.subjectId,
        message:
          i.status === 'error'
            ? `Process ${i.instanceId} is in error state (${i.definitionKey} v${i.definitionVersion})`
            : `Process ${i.instanceId} terminated with outcome '${i.outcome}' (${i.definitionKey} v${i.definitionVersion})`,
      })
    }
  }

  // 2. Poisoned/stuck command receipts.
  const receipts = (await execute`
    select command_id, aggregate_id::text, message
    from workflow_command_receipt where outcome = 'pending' order by created_at
  `) as any[]
  for (const r of receipts) {
    anomalies.push({
      kind: 'pending-receipt',
      severity: 'critical',
      instanceId: null,
      subjectId: null,
      message: `Command receipt ${r.command_id} is stuck 'pending'${r.message ? ` — ${r.message}` : ''}`,
    })
  }

  // 3. Ready engine tasks without a canonical correlation.
  const uncorrelated = (await execute`
    select t.id::text as id, t.process_instance_id::text as pid, t.name, t.status
    from tasks t
    left join workflow_task_correlation c on c.workflow_task_id = t.id::text
    where t.status in ('ready', 'reserved', 'in_progress')
      and c.workflow_task_id is null
  `) as any[]
  for (const t of uncorrelated) {
    anomalies.push({
      kind: 'ready-task-uncorrelated',
      severity: 'warning',
      instanceId: t.pid,
      subjectId: null,
      message: `Engine task ${t.name} (${t.id}) is ${t.status} but has no canonical correlation`,
    })
  }

  // 4. Correlations whose canonical task no longer exists.
  const danglingApp = (await execute`
    select c.workflow_task_id, c.application_task_id::text
    from workflow_task_correlation c
    left join task t on t.id = c.application_task_id
    where t.id is null
  `) as any[]
  for (const c of danglingApp) {
    anomalies.push({
      kind: 'correlation-dangling-app-task',
      severity: 'warning',
      instanceId: null,
      subjectId: c.application_task_id,
      message: `Correlation ${c.workflow_task_id} references missing canonical task ${c.application_task_id}`,
    })
  }

  // 5. Correlations whose engine task no longer exists.
  const danglingEngine = (await execute`
    select c.workflow_task_id, c.application_task_id::text
    from workflow_task_correlation c
    left join tasks t on t.id::text = c.workflow_task_id
    where t.id is null
  `) as any[]
  for (const c of danglingEngine) {
    anomalies.push({
      kind: 'correlation-dangling-workflow-task',
      severity: 'warning',
      instanceId: null,
      subjectId: null,
      message: `Correlation ${c.workflow_task_id} references a missing engine task (canonical ${c.application_task_id})`,
    })
  }

  // 6. Open jobs whose token is already completed/skipped.
  const staleJobs = (await execute`
    select
      j.id::text as job_id,
      j.type,
      j.status,
      j.process_instance_id::text as pid,
      t.id::text as token_id,
      t.outcome as token_outcome
    from jobs j
    join tokens t on t.id = j.token_id
    where j.status in ('pending', 'locked')
      and t.outcome in ('completed', 'skipped')
  `) as any[]
  for (const j of staleJobs) {
    anomalies.push({
      kind: 'open-job-on-closed-token',
      severity: 'warning',
      instanceId: j.pid,
      subjectId: null,
      message: `Job ${j.job_id} (${j.type}) is ${j.status} but token ${j.token_id} is already ${j.token_outcome}`,
    })
  }

  // 7. Multiple active instances for the same subject.
  const multi = (await execute`
    select subject_type, subject_id, count(*)::int as c
    from process_instances
    where status = 'active' and subject_id is not null
    group by subject_type, subject_id
    having count(*) > 1
  `) as any[]
  for (const m of multi) {
    anomalies.push({
      kind: 'multiple-active-instances',
      severity: 'warning',
      instanceId: null,
      subjectId: m.subject_id,
      message: `Subject ${m.subject_type}:${m.subject_id} has ${m.c} active instances`,
    })
  }
}
