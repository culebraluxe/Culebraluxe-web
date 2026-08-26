import { getProperties } from '@/db/properties'
import { getMarketingContent } from '@/db/marketing-content'
import { buildHomeContent } from '@/lib/marketing-content'
import { SiteHeader } from '@/components/site-header'
import { Hero } from '@/components/hero'
import { FeaturedProperties } from '@/components/featured-properties'
import { HomeProperties } from '@/components/home-properties'
import { Services } from '@/components/services'
import { Culture } from '@/components/culture'
import { About } from '@/components/about'
import { Contact } from '@/components/contact'
import { SiteFooter } from '@/components/site-footer'

export const dynamic = 'force-dynamic'

export default async function Page() {
  // DB-HARDEN-01C — failure-safe orchestration. Each public read returns its
  // own Result; a failure on one optional source never poisons the others or
  // rejects the whole page. On failure we degrade only that module.
  const [propertiesResult, contentResult] = await Promise.all([
    getProperties({ publicOnly: true }),
    getMarketingContent(),
  ])

  const properties = propertiesResult.ok ? propertiesResult.data : null
  const home = contentResult.ok ? buildHomeContent(contentResult.data) : undefined
  const featured = properties?.filter((property) => property.featured) ?? []

  return (
    <>
      <SiteHeader />
      <main>
        {home?.hero ? <Hero content={home.hero} /> : null}
        {properties && properties.length > 0 ? (
          <>
            <FeaturedProperties properties={featured} />
            <HomeProperties properties={properties} />
          </>
        ) : null}
        {home?.buyers && home?.sellers ? (
          <Services buyers={home.buyers} sellers={home.sellers} />
        ) : null}
        {home?.culture ? <Culture content={home.culture} /> : null}
        {home?.about ? <About content={home.about} /> : null}
        {home?.contact ? <Contact content={home.contact} /> : null}
      </main>
      <SiteFooter />
    </>
  )
}
