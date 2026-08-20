// ---------------------------------------------------------------------------
// Task materialization reconciliation — the V1 caller for materializeEngineTask.
//
// Discovers active engine human tasks and materializes each exactly once into a
// canonical CulebraLuxe operational task via deterministic correlation. Safe on
// repeated invocation. No scheduler here.
// ---------------------------------------------------------------------------

export type EngineTaskView = {
  workflowTaskId: string
  title: string
  subjectType: string
  subjectId: string
  dealId: string | null
}

export async function reconcileTaskMaterializationCore(
  tasks: EngineTaskView[],
  materialize: (
    t: EngineTaskView,
  ) => Promise<{ applicationTaskId: string; created: boolean }>,
): Promise<{ materialized: number; skipped: number }> {
  let materialized = 0
  let skipped = 0
  for (const t of tasks) {
    const r = await materialize(t)
    if (r.created) materialized += 1
    else skipped += 1
  }
  return { materialized, skipped }
}

export async function reconcileTaskMaterialization(): Promise<{
  materialized: number
  skipped: number
}> {
  const { engineConfigured, engineSql } = await import('./engine-client')
  if (!engineConfigured()) return { materialized: 0, skipped: 0 }

  const { materializeEngineTask } = await import('./task-materialization')
  const esql = engineSql()

  const rows = await esql`
    select
      t.id as workflow_task_id,
      t.name as title,
      pi.subject_type,
      pi.subject_id,
      pi.subject_id as deal_id
    from tasks t
    join process_instances pi on pi.id = t.process_instance_id
    where pi.subject_type = 'deal'
      and t.status in ('ready', 'reserved', 'in_progress')
  `

  const tasks: EngineTaskView[] = (rows as Array<{
    workflow_task_id: string
    title: string
    subject_type: string
    subject_id: string
    deal_id: string
  }>).map((r) => ({
    workflowTaskId: r.workflow_task_id,
    title: r.title,
    subjectType: r.subject_type,
    subjectId: r.subject_id,
    dealId: r.deal_id,
  }))

  return reconcileTaskMaterializationCore(tasks, (t) =>
    materializeEngineTask({
      workflowTaskId: t.workflowTaskId,
      title: t.title,
      subjectType: t.subjectType,
      subjectId: t.subjectId,
      dealId: t.dealId ?? undefined,
    }),
  )
}
