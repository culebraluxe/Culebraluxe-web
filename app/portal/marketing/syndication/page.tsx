import { SyndicationWorkbench } from '@/components/portal/marketing/syndication-workbench'
import { listListingSources, listPlacements } from '@/db/syndication'

export const dynamic = 'force-dynamic'

export default async function SyndicationPage() {
  const [sources, placements] = await Promise.all([
    listListingSources(),
    listPlacements(),
  ])

  return <SyndicationWorkbench sources={sources} placements={placements} />
}
