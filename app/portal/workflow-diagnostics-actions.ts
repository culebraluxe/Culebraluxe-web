'use server'

import { inspectInstance } from '@/workflow_app/diagnostics'
import type { InstanceDetail } from '@/workflow_app/diagnostics'

// Read-only workflow diagnostics detail loader for the IT support page
// (CRM-14N). The snapshot is rendered server-side; individual instance
// technical detail is fetched lazily when a support operator expands a row.
// No engine changes, no XML changes, no workflow mutation.

export async function loadWorkflowInstanceDetail(
  instanceId: string
): Promise<InstanceDetail | null> {
  return inspectInstance(instanceId)
}
