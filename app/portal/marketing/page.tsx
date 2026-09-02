import { MarketingDashboard } from '@/components/portal/marketing/marketing-dashboard'
import {
  getMarketingDashboard,
  listPlacements,
  listRecentSyndicationEvents,
} from '@/db/syndication'

export const dynamic = 'force-dynamic'

export default async function MarketingDashboardPage() {
  const [snapshot, placements, events] = await Promise.all([
    getMarketingDashboard(),
    listPlacements(),
    listRecentSyndicationEvents(18),
  ])

  return (
    <MarketingDashboard
      snapshot={snapshot}
      placements={placements}
      events={events}
    />
  )
}
