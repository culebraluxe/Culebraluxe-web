import { getProperties } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { FavoritesView } from '@/components/property/favorites-view'

export const dynamic = 'force-dynamic'

export default async function FavoritesPage() {
  // DB-HARDEN-01C — public read: degrade to empty on failure (no crash).
  const result = await getProperties({ publicOnly: true })
  const properties = result.ok ? result.data : []

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
