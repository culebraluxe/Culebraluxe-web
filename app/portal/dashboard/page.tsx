import { Dashboard } from "@/components/portal/dashboard"
import { getClients } from "@/db/clients"
import { getDeals } from "@/db/deals"

export const dynamic = "force-dynamic"

export default async function DashboardPage() {
  const [clients, deals] = await Promise.all([
    getClients(),
    getDeals(),
  ])

  return (
    <Dashboard
      clients={clients}
      deals={deals}
    />
  )
}