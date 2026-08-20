import { WorkflowsList } from "@/components/portal/workflows-list"
import { getWorkflowSummaries } from "@/workflow_app/read-service"
import { engineConfigured } from "@/workflow_app/engine-client"

export const dynamic = "force-dynamic"

export default async function WorkflowsPage() {
  const configured = engineConfigured()
  const summaries = configured ? await getWorkflowSummaries() : []

  return <WorkflowsList configured={configured} summaries={summaries} />
}
