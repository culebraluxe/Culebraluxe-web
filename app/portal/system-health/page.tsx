import { SystemHealth } from "@/components/portal/system-health"
import { WorkflowDiagnostics } from "@/components/portal/workflow-diagnostics"
import { getSystemHealth } from "@/db/system-health"
import { getWorkflowDiagnosticsSnapshot } from "@/workflow_app/diagnostics"

export const dynamic = "force-dynamic"

export default async function SystemHealthPage() {
  const [health, diagnostics] = await Promise.all([
    getSystemHealth(),
    getWorkflowDiagnosticsSnapshot(),
  ])

  return (
    <>
      <SystemHealth health={health} />
      <WorkflowDiagnostics diagnostics={diagnostics} />
    </>
  )
}
