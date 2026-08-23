// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Read-only Dashboard preview. Reuses the exact same data functions and
// presentational components as /portal/dashboard (Dashboard,
// WorkflowDashboardCard) — both are pure read projections. No mutation
// surface.
// ═══════════════════════════════════════════════════════════════════════════

import { PageHeader } from "@/components/portal/page-header"
import { Dashboard } from "@/components/portal/dashboard"
import { WorkflowDashboardCard } from "@/components/portal/workflow-dashboard-card"
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
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portal Preview"
        title="Dashboard"
        subtitle="Read-only preview — same live projection as /portal/dashboard."
      />
      <WorkflowDashboardCard summaries={workflowSummaries} />
      <Dashboard clients={clients} deals={deals} snapshot={snapshot} />
    </div>
  )
}
