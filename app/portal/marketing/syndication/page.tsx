import { SyndicationWorkbench } from '@/components/portal/marketing/syndication-workbench'
import { listListingSources, listPlacements } from '@/db/syndication'
import { expireStalePlacements } from '@/db/syndication-expire'
import { facebookReadiness } from '@/lib/syndication/env'

export const dynamic = 'force-dynamic'

export default async function SyndicationPage() {
  await expireStalePlacements()
  const [sources, placements] = await Promise.all([
    listListingSources(),
    listPlacements(),
  ])

  return (
    <SyndicationWorkbench
      sources={sources}
      placements={placements}
      facebook={facebookReadiness()}
    />
  )
}
