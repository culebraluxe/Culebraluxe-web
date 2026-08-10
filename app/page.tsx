import { SiteHeader } from '@/components/site-header'
import { Hero } from '@/components/hero'
import { FeaturedProperties } from '@/components/featured-properties'
import { HomeProperties } from '@/components/home-properties'
import { Services } from '@/components/services'
import { Culture } from '@/components/culture'
import { About } from '@/components/about'
import { Contact } from '@/components/contact'
import { SiteFooter } from '@/components/site-footer'

export default function Page() {
  return (
    <>
      <SiteHeader />
      <main>
        <Hero />
        <FeaturedProperties />
        <HomeProperties />
        <Services />
        <Culture />
        <About />
        <Contact />
      </main>
      <SiteFooter />
    </>
  )
}
