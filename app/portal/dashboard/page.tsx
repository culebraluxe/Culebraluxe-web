import { Dashboard } from "@/components/portal/dashboard"
import { getClients } from "@/db/clients"
import { getDeals } from "@/db/deals"
import { getDashboardSnapshot } from "@/db/dashboard"
import { getWorkflowSummaries } from "@/workflow_app/read-service"

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
