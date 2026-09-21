import { SystemHealth } from "@/components/portal/system-health"
import { WorkflowDiagnostics } from "@/components/portal/workflow-diagnostics"
import { getSystemHealth } from "@/legacy/db/system-health"
import { getWorkflowDiagnosticsSnapshot } from "@/legacy/workflow_app/diagnostics"
import { getEnvironmentReadiness } from "@/lib/environment-readiness"

export const dynamic = "force-dynamic"

export default async function SystemHealthPage() {
  const [health, diagnostics, environment] = await Promise.all([
    getSystemHealth(),
    getWorkflowDiagnosticsSnapshot(),
    Promise.resolve(getEnvironmentReadiness()),
  ])

  return (
    <>
      <SystemHealth health={health} environmentReadiness={environment} />
      <WorkflowDiagnostics diagnostics={diagnostics} />
    </>
  )
}
