'use server'

import { withServerErrorCapture } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

export type WorkflowDiagnosticsDetail = {
  instance: Record<string, unknown>
  variables: unknown | null
  nodeLabels: Record<string, string>
  tokens: unknown[]
  tasks: unknown[]
  jobs: unknown[]
  events: unknown[]
  correlations: unknown[]
  commands: unknown[]
}

async function loadWorkflowInstanceDetailHandler(
  instanceId: string,
): Promise<WorkflowDiagnosticsDetail | null> {
  if (!instanceId.trim()) return null
  const result = await rustApiRead<WorkflowDiagnosticsDetail>(
    (`/v1/support/workflow-diagnostics/${encodeURIComponent(instanceId)}`) as `/v1/${string}`,
  )
  return result.value
}

export const loadWorkflowInstanceDetail = withServerErrorCapture(
  'portal/workflow-diagnostics-actions.loadWorkflowInstanceDetail',
  loadWorkflowInstanceDetailHandler,
)
