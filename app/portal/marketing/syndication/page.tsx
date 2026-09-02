import { SyndicationWorkbench } from '@/components/portal/marketing/syndication-workbench'
import { listListingSources, listPlacements, listSightings } from '@/db/syndication'
import { expireStalePlacements } from '@/db/syndication-expire'

export const dynamic = 'force-dynamic'

export default async function SyndicationPage() {
  await expireStalePlacements()
  const [sources, placements, sightings] = await Promise.all([
    listListingSources(),
    listPlacements(),
    listSightings(),
  ])

  return <SyndicationWorkbench sources={sources} placements={placements} sightings={sightings} />
}

