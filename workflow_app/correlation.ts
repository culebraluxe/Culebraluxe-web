import { sql } from '../db/client'

// ---------------------------------------------------------------------------
// Workflow <-> application task correlation.
//
// The canonical CulebraLuxe `task` stays the user-facing work item; the engine
// keeps its own runtime task state. This table is the deterministic
// correlation key between the two (migration 019).
//
//   - correlateTask is idempotent (on conflict do nothing), so a retry cannot
//     create a duplicate visible task.
//   - workflow_task_id is the deterministic engine identity (PK).
//   - application_task_id is UNIQUE, so one canonical task maps to at most one
//     engine task.
//   - findApplicationTaskId returns null when no correlation exists (explicit
//     "missing" behavior); a stale correlation is detected by the application
//     task's own status (completed/cancelled).
// ---------------------------------------------------------------------------

export type TaskCorrelation = {
  workflowTaskId: string
  applicationTaskId: string
  subjectType: string
  subjectId: string
}

export async function correlateTask(
  workflowTaskId: string,
  applicationTaskId: string,
  subjectType: string,
  subjectId: string,
): Promise<void> {
  await sql`
    insert into workflow_task_correlation (
      workflow_task_id, application_task_id, subject_type, subject_id
    ) values (
      ${workflowTaskId}, ${applicationTaskId}, ${subjectType}, ${subjectId}
    )
    on conflict (workflow_task_id) do nothing
  `
}

export async function findApplicationTaskId(
  workflowTaskId: string,
): Promise<string | null> {
  const rows = await sql`
    select application_task_id
    from workflow_task_correlation
    where workflow_task_id = ${workflowTaskId}
    limit 1
  `
  return (rows[0] as { application_task_id: string } | undefined)
    ?.application_task_id ?? null
}

export async function findWorkflowTaskId(
  applicationTaskId: string,
): Promise<string | null> {
  const rows = await sql`
    select workflow_task_id
    from workflow_task_correlation
    where application_task_id = ${applicationTaskId}
    limit 1
  `
  return (rows[0] as { workflow_task_id: string } | undefined)
    ?.workflow_task_id ?? null
}
