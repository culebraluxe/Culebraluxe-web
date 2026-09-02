import { MarketingDashboard } from '@/components/portal/marketing/marketing-dashboard'
import {
  getMarketingDashboard,
  listPlacements,
  listRecentSyndicationEvents,
  listSightings,
} from '@/db/syndication'
import { expireStalePlacements } from '@/db/syndication-expire'
import { facebookReadiness } from '@/lib/syndication/env'

export const dynamic = 'force-dynamic'

export default async function MarketingDashboardPage() {
  await expireStalePlacements()
  const [snapshot, placements, events, sightings] = await Promise.all([
    getMarketingDashboard(),
    listPlacements(),
    listRecentSyndicationEvents(18),
    listSightings(),
  ])

  return (
    <MarketingDashboard
      snapshot={snapshot}
      placements={placements}
      events={events}
      sightings={sightings}
      facebook={facebookReadiness()}
    />
  )
}
