'use server'

import { withServerErrorCapture } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'
import type { InstanceDetail } from '@/lib/workflow-diagnostics-types'

async function loadWorkflowInstanceDetailHandler(
  instanceId: string,
): Promise<InstanceDetail | null> {
  if (!instanceId.trim()) return null
  const result = await rustApiRead<InstanceDetail>(
    (`/v1/support/workflow-diagnostics/${encodeURIComponent(instanceId)}`) as `/v1/${string}`,
  )
  return result.value
}

export const loadWorkflowInstanceDetail = withServerErrorCapture(
  'portal/workflow-diagnostics-actions.loadWorkflowInstanceDetail',
  loadWorkflowInstanceDetailHandler,
)
