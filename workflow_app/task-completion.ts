import { WorkflowEngine } from '../workflow_engine/lib/workflow/engine'
import { createApplicationPort } from './application-port'
import { engineSql } from './engine-client'
import { findWorkflowTaskId } from './correlation'

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
// ---------------------------------------------------------------------------

export type CompleteWorkflowTaskInput = {
  applicationTaskId: string
  userId: string
  transitionName?: string
  formData?: Record<string, any>
}

export async function completeWorkflowTask(
  input: CompleteWorkflowTaskInput,
): Promise<void> {
  const workflowTaskId = await findWorkflowTaskId(input.applicationTaskId)
  if (!workflowTaskId) {
    throw new Error(
      `No workflow task correlates to application task ${input.applicationTaskId}`,
    )
  }

  const engine = new WorkflowEngine(engineSql(), { app: createApplicationPort() })
  await engine.completeTask({
    taskId: workflowTaskId,
    userId: input.userId,
    formData: input.formData ?? {},
    transitionName: input.transitionName,
  })
}
