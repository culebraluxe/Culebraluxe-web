import { getProperties } from '@/db/properties'
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
  const properties = await getProperties()

  const featured = properties.filter((property) => property.featured)

  return (
    <>
      <SiteHeader />
      <main>
        <Hero />
        <FeaturedProperties properties={featured} />
        <HomeProperties properties={properties} />
        <Services />
        <Culture />
        <About />
        <Contact />
      </main>
      <SiteFooter />
    </>
  )
}
