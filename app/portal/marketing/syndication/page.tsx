import { SyndicationWorkbench } from '@/components/portal/marketing/syndication-workbench'
import { listListingSources, listPlacements, listRecentSyndicationEvents, listSightings } from '@/db/syndication'
import { expireStalePlacements } from '@/db/syndication-expire'
import { facebookReadiness } from '@/lib/syndication/env'

export const dynamic = 'force-dynamic'

export default async function SyndicationPage() {
  await expireStalePlacements()
  const [sources, placements, sightings, activity] = await Promise.all([
    listListingSources(),
    listPlacements(),
    listSightings(),
    listRecentSyndicationEvents(300),
  ])

  return (
    <SyndicationWorkbench
      sources={sources}
      placements={placements}
      sightings={sightings}
      activity={activity}
      facebook={facebookReadiness()}
    />
  )
}
