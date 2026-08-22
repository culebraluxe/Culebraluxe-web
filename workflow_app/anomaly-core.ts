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

// ---------------------------------------------------------------------------
// Instance health classification (ENG-15).
//
// One deterministic mapping from the operational anomaly rules to a
// per-instance health classification. `classifyInstanceHealth` is pure: the
// caller supplies the instance's canonical status/outcome plus the anomalies
// that fired for it (from the global sweep or an instance-scoped collection),
// and receives the classification plus the reasons behind it. This is the
// single source of the stuck/healthy vocabulary; the query contract and any
// future operator surface must go through it rather than inventing their own.
//
// Ordering is deliberate:
//   1. failed   — critical failure rules dominate everything
//   2. terminal — a normally-finished instance is never "stuck"
//   3. stuck    — operational anomaly rules that halt progress on an active
//                 instance (wedged, stale lease, job on closed token, orphan
//                 token, poisoned command receipt)
//   4. healthy  — active with no operational anomaly
// ---------------------------------------------------------------------------

export type WorkflowHealthClassification =
  | 'healthy'
  | 'stuck'
  | 'failed'
  | 'terminal'

export type WorkflowHealth = {
  classification: WorkflowHealthClassification
  /** Human-readable reasons behind the classification (anomaly rule messages). */
  reasons: string[]
  /** The anomalies that fired for this instance (empty when healthy). */
  anomalies: WorkflowAnomaly[]
}

/** Failure rules — always classify the instance as failed. */
export const FAILED_HEALTH_KINDS = new Set([
  'failed-process',
  'error-instance-missing-outcome',
])

/** Stuck rules — an active instance halted by an operational anomaly. */
export const STUCK_HEALTH_KINDS = new Set([
  'wedged-instance',
  'stale-locked-job',
  'open-job-on-closed-token',
  'orphan-token',
  'pending-receipt',
])

/** Normally-terminal instance statuses (never classified as stuck). */
const TERMINAL_STATUSES = new Set(['completed', 'cancelled', 'aborted'])

export function classifyInstanceHealth(input: {
  status: string
  outcome: string | null
  anomalies: WorkflowAnomaly[]
}): WorkflowHealth {
  const anomalies = input.anomalies
  const failed = anomalies.filter((a) => FAILED_HEALTH_KINDS.has(a.kind))
  if (failed.length > 0) {
    return {
      classification: 'failed',
      reasons: failed.map((a) => a.message),
      anomalies,
    }
  }
  if (TERMINAL_STATUSES.has(input.status)) {
    return {
      classification: 'terminal',
      reasons: [`Instance reached terminal disposition '${input.status}'`],
      anomalies,
    }
  }
  const stuck = anomalies.filter((a) => STUCK_HEALTH_KINDS.has(a.kind))
  if (stuck.length > 0) {
    return {
      classification: 'stuck',
      reasons: stuck.map((a) => a.message),
      anomalies,
    }
  }
  return { classification: 'healthy', reasons: [], anomalies }
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

  // 8. Stale locked job — a job whose lease has expired (its worker died or
  // was partitioned). Deterministically detectable from the lease column.
  // Recovery is reclaimStaleJobs (CRM-14F); surfacing here keeps it visible
  // until an operator/poller reclaims it.
  const staleLocks = (await execute`
    select
      j.id::text as job_id,
      j.type,
      j.process_instance_id::text as pid,
      j.locked_by,
      j.locked_until::text as locked_until
    from jobs j
    where j.status = 'locked'
      and j.locked_until < now()
  `) as any[]
  for (const j of staleLocks) {
    anomalies.push({
      kind: 'stale-locked-job',
      severity: 'warning',
      instanceId: j.pid,
      subjectId: null,
      message: `Job ${j.job_id} (${j.type}) is locked by ${j.locked_by} past its lease (${j.locked_until})`,
    })
  }

  // 9. Wedged instance — active instance with no active tokens and no
  // pending/locked jobs: it completed its tokens but was never terminalized
  // (an atomicity artifact). Safe auto-repair is NOT available for this class
  // (disposition is a judgment call); surface it for operator review.
  const wedged = (await execute`
    select pi.id::text as instance_id, pd.key, pd.version
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.status = 'active'
      and not exists (
        select 1 from tokens t
        where t.process_instance_id = pi.id and t.status = 'active'
      )
      and not exists (
        select 1 from jobs j
        where j.process_instance_id = pi.id and j.status in ('pending', 'locked')
      )
  `) as any[]
  for (const w of wedged) {
    anomalies.push({
      kind: 'wedged-instance',
      severity: 'warning',
      instanceId: w.instance_id,
      subjectId: null,
      message: `Process ${w.instance_id} (${w.key} v${w.version}) is active but has no active tokens or pending work — was never terminalized`,
    })
  }

  // 10. Orphan token — an active token whose process instance no longer
  // exists (should be prevented by FK, but a partition/import edge could
  // surface it). Also catches active tokens on terminal instances.
  const orphan = (await execute`
    select t.id::text as token_id, t.process_instance_id::text as pid, t.node_id
    from tokens t
    left join process_instances pi on pi.id = t.process_instance_id
    where t.status = 'active'
      and (pi.id is null or pi.status != 'active')
  `) as any[]
  for (const o of orphan) {
    anomalies.push({
      kind: 'orphan-token',
      severity: 'warning',
      instanceId: o.pid,
      subjectId: null,
      message: `Token ${o.token_id} at '${o.node_id}' is active but its process is missing or not active`,
    })
  }

  // 11. Error instance missing an outcome — an 'error'-status process with no
  // recorded terminal outcome (impossible-state detection for operators).
  const errorNoOutcome = (await execute`
    select pi.id::text as instance_id, pd.key, pd.version
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.status = 'error' and pi.outcome is null
  `) as any[]
  for (const e of errorNoOutcome) {
    anomalies.push({
      kind: 'error-instance-missing-outcome',
      severity: 'critical',
      instanceId: e.instance_id,
      subjectId: null,
      message: `Process ${e.instance_id} (${e.key} v${e.version}) is 'error' with no terminal outcome`,
    })
  }
}
