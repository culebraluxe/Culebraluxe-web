// ---------------------------------------------------------------------------
// Operational task materialization — turns ONE engine human task into ONE
// canonical CulebraLuxe operational task, with deterministic correlation.
//
// The canonical CulebraLuxe `task` stays the user-facing work item; the engine
// keeps its own runtime task state. `workflow_task_id` is the deterministic
// engine identity; `workflow_task_correlation.application_task_id` is UNIQUE,
// so a retry can never create a duplicate visible task.
//
// No scheduler and no notifications here: the caller invokes this exactly once
// per engine task it first observes (Portal read/reconciliation path).
// ---------------------------------------------------------------------------

export type MaterializeTaskInput = {
  workflowTaskId: string
  title: string
  detail?: string
  subjectType: string
  subjectId: string
  dealId?: string
  personId?: string
  propertyId?: string
  dueAt?: string
}

export type MaterializeTaskResult = {
  applicationTaskId: string
  created: boolean
}

export type MaterializeDeps = {
  findCorrelation: (workflowTaskId: string) => Promise<string | null>
  createTask: (input: {
    title: string
    detail?: string
    dealId?: string
    personId?: string
    propertyId?: string
    dueAt?: string
  }) => Promise<{ id: string }>
  correlate: (
    workflowTaskId: string,
    applicationTaskId: string,
    subjectType: string,
    subjectId: string,
  ) => Promise<void>
}

// Pure, testable core. The DB wiring lives in `materializeEngineTask` below.
export async function materializeEngineTaskCore(
  input: MaterializeTaskInput,
  deps: MaterializeDeps,
): Promise<MaterializeTaskResult> {
  const existing = await deps.findCorrelation(input.workflowTaskId)
  if (existing) {
    return { applicationTaskId: existing, created: false }
  }

  const task = await deps.createTask({
    title: input.title,
    detail: input.detail,
    dealId: input.dealId,
    personId: input.personId,
    propertyId: input.propertyId,
    dueAt: input.dueAt,
  })

  await deps.correlate(
    input.workflowTaskId,
    task.id,
    input.subjectType,
    input.subjectId,
  )

  return { applicationTaskId: task.id, created: true }
}

export async function materializeEngineTask(
  input: MaterializeTaskInput,
): Promise<MaterializeTaskResult> {
  const { createTask } = await import('../db/tasks')
  const { correlateTask, findApplicationTaskId } = await import('./correlation')
  return materializeEngineTaskCore(input, {
    findCorrelation: findApplicationTaskId,
    createTask: async (i) => {
      const t = await createTask({
        title: i.title,
        detail: i.detail,
        dealId: i.dealId,
        personId: i.personId,
        propertyId: i.propertyId,
        dueAt: i.dueAt,
        taskKind: 'human',
      })
      return { id: t.id }
    },
    correlate: correlateTask,
  })
}
