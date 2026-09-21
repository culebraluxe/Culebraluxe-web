// ---------------------------------------------------------------------------
// DEV workflow reset harness (CRM-14M).
//
// Resets the workflow ENGINE runtime state (plus the workflow correlation and
// command-receipt tables) back to a clean slate. DEV-ONLY and destructive:
//
//   - refuses unless APP_ENV === 'development'
//   - never deletes process_definitions (the deployed definition stays)
//   - never touches canonical business data except the canonical `task` rows
//     that were materialized BY the workflow (identified via the correlation
//     table), so no orphan Portal tasks are left behind
//
// Delete order is child-before-parent and is auditable via RESET_STEPS.
// ---------------------------------------------------------------------------

export type ResetStep = { table: string; statement: string }
export type ResetResult = { table: string; deleted: number }
export type RawSqlExecutor = (sqlText: string) => Promise<unknown[]>

export function assertDevResetAllowed(appEnv: string | undefined): void {
  if (appEnv !== 'development') {
    throw new Error(
      `Workflow reset is DEV-only; refused for APP_ENV=${appEnv ?? 'undefined'}.`,
    )
  }
}

/** Ordered delete steps (children first; definitions preserved). */
export const RESET_STEPS: ResetStep[] = [
  {
    table: 'task (materialized canonical)',
    statement:
      'delete from task where id in (select application_task_id from workflow_task_correlation) returning id',
  },
  { table: 'workflow_task_correlation', statement: 'delete from workflow_task_correlation returning workflow_task_id' },
  { table: 'workflow_command_receipt', statement: 'delete from workflow_command_receipt returning command_id' },
  { table: 'process_events', statement: 'delete from process_events returning id' },
  { table: 'process_commands', statement: 'delete from process_commands returning id' },
  { table: 'jobs', statement: 'delete from jobs returning id' },
  { table: 'tasks', statement: 'delete from tasks returning id' },
  { table: 'tokens', statement: 'delete from tokens returning id' },
  { table: 'process_instances', statement: 'delete from process_instances returning id' },
]

/**
 * Execute the ordered delete steps against a raw-SQL runner. Each statement
 * uses `returning` so the number of deleted rows is observable. The runner is
 * injected so the order and row counts are unit-testable without a database.
 */
export async function resetDevWorkflowsCore(
  exec: RawSqlExecutor,
  steps: ResetStep[] = RESET_STEPS,
): Promise<ResetResult[]> {
  const results: ResetResult[] = []
  for (const step of steps) {
    const rows = await exec(step.statement)
    results.push({ table: step.table, deleted: rows.length })
  }
  return results
}
