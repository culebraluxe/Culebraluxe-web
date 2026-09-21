import { Dashboard } from "@/components/portal/dashboard"
import { getClients } from "@/legacy/db/clients"
import { getDeals } from "@/legacy/db/deals"
import { getDashboardSnapshot } from "@/legacy/db/dashboard"
import { getWorkflowSummaries } from "@/legacy/workflow_app/read-service"

export const dynamic = "force-dynamic"

export default async function DashboardPage() {
  const [clients, deals, snapshot, workflowSummaries] = await Promise.all([
    getClients(),
    getDeals(),
    getDashboardSnapshot(),
    getWorkflowSummaries(),
  ])

  return (
    <Dashboard
      clients={clients}
      deals={deals}
      snapshot={snapshot}
      workflowSummaries={workflowSummaries}
    />
  )
}
