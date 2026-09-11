import { DealsPortfolio } from "@/components/portal/deals-portfolio"
import { ContractsPortfolio } from "@/components/portal/contracts-portfolio"
import { DealCreatePanel } from "@/components/portal/write/deal-create-panel"
import { getDeals, listDealableProperties } from "@/db/deals"
import { getSettingsUsers } from "@/db/settings-auth"
import { listContractPortfolio } from "@/lib/contract-reads"

export const dynamic = "force-dynamic"

export default async function DealsPage() {
  const [deals, properties, users, contracts] = await Promise.all([
    getDeals(),
    listDealableProperties(),
    getSettingsUsers(),
    listContractPortfolio(),
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
      <ContractsPortfolio
        contracts={contracts.ok ? contracts.data : []}
        readFailed={!contracts.ok}
      />
    </>
  )
}
