import { getProperties } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { FavoritesView } from '@/components/property/favorites-view'

export const dynamic = 'force-dynamic'

export default async function FavoritesPage() {
  const properties = await getProperties({ publicOnly: true })

  return (
    <>
      <SiteHeader />

      <main>
        <FavoritesView properties={properties} />
      </main>

      <SiteFooter />
    </>
  )
}
