import { Dashboard } from "@/components/portal/dashboard"
import { getClients } from "@/db/clients"
import { getDeals } from "@/db/deals"
import { getDashboardSnapshot } from "@/db/dashboard"

export const dynamic = "force-dynamic"

export default async function DashboardPage() {
  const [clients, deals, snapshot] = await Promise.all([
    getClients(),
    getDeals(),
    getDashboardSnapshot(),
  ])

  return (
    <Dashboard
      clients={clients}
      deals={deals}
      snapshot={snapshot}
    />
  )
}