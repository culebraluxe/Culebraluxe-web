// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Read-only Dashboard preview. Reuses the exact same data functions and
// presentational Dashboard as /portal/dashboard. Pure read projection.
// No mutation surface.
// ═══════════════════════════════════════════════════════════════════════════

import { Dashboard } from "@/components/portal/dashboard"
import { getClients } from "@/db/clients"
import { getDeals } from "@/db/deals"
import { getDashboardSnapshot } from "@/db/dashboard"
import { getWorkflowSummaries } from "@/workflow_app/read-service"

export async function ReviewDashboard() {
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
