import { DealsPortfolio } from "@/components/portal/deals-portfolio"
import { getDeals } from "@/db/deals"

export const dynamic = "force-dynamic"

export default async function DealsPage() {
  const deals = await getDeals()

  return <DealsPortfolio deals={deals} />
}