import { DealsPortfolio } from "@/components/portal/deals-portfolio"
import { DealCreatePanel } from "@/components/portal/write/deal-create-panel"
import { getDeals, listDealableProperties } from "@/db/deals"
import { getSettingsUsers } from "@/db/settings-auth"

export const dynamic = "force-dynamic"

export default async function DealsPage() {
  const [deals, properties, users] = await Promise.all([
    getDeals(),
    listDealableProperties(),
    getSettingsUsers(),
  ])

  return (
    <>
      <DealCreatePanel
        properties={properties}
        users={users
          .filter((user) => user.active)
          .map((user) => ({
            id: user.id,
            displayName: user.displayName,
            email: user.email,
          }))}
      />
      <DealsPortfolio deals={deals} />
    </>
  )
}
