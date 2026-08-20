// ---------------------------------------------------------------------------
// Human-task completion seam (CRM-14I / Story 147).
//
// The application owns the canonical user-facing task; the engine owns the
// runtime task. Completing a workflow task means:
//
//   1. the canonical task is completed through the normal application service
//      (db/portal-writes.completeTask),
//   2. the engine task is completed through the engine's runtime method with
//      the transition chosen from the deployed definition.
//
// This module is the (2) step: it resolves the canonical task id back to the
// engine task via the correlation table and completes it through the engine.
// The transition name is taken from the deployed XML; when omitted, the engine
// uses the node's first declared transition (the normal advancement path).
//
// The DB/engine wiring is imported dynamically so `completeWorkflowTaskCore`
// stays importable in unit tests without a database (same pattern as
// task-materialization.ts and closing-timer.ts).
// ---------------------------------------------------------------------------

export type CompleteWorkflowTaskInput = {
  applicationTaskId: string
  userId: string
  transitionName?: string
  formData?: Record<string, any>
}

export type CompleteWorkflowTaskDeps = {
  findWorkflowTaskId: (applicationTaskId: string) => Promise<string | null>
  completeEngineTask: (
    workflowTaskId: string,
    input: CompleteWorkflowTaskInput,
  ) => Promise<void>
}

// Pure, testable core. The DB/engine wiring lives in `completeWorkflowTask`.
export async function completeWorkflowTaskCore(
  input: CompleteWorkflowTaskInput,
  deps: CompleteWorkflowTaskDeps,
): Promise<{ workflowTaskId: string }> {
  const workflowTaskId = await deps.findWorkflowTaskId(input.applicationTaskId)
  if (!workflowTaskId) {
    throw new Error(
      `No workflow task correlates to application task ${input.applicationTaskId}`,
    )
  }
  await deps.completeEngineTask(workflowTaskId, input)
  return { workflowTaskId }
}

export async function completeWorkflowTask(
  input: CompleteWorkflowTaskInput,
): Promise<void> {
  const { findWorkflowTaskId } = await import('./correlation')
  const { engineSql } = await import('./engine-client')
  const { createApplicationPort } = await import('./application-port')
  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')

  await completeWorkflowTaskCore(input, {
    findWorkflowTaskId,
    completeEngineTask: async (workflowTaskId, i) => {
      const engine = new WorkflowEngine(engineSql(), { app: createApplicationPort() })
      await engine.completeTask({
        taskId: workflowTaskId,
        userId: i.userId,
        formData: i.formData ?? {},
        transitionName: i.transitionName,
      })
    },
  })
}
