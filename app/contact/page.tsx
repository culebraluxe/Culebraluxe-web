import type { Metadata } from 'next'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Contact } from '@/components/contact'

export const metadata: Metadata = {
  title: 'Contact — CulebraLuxe',
  description:
    'Begin a quiet conversation with CulebraLuxe about buying or selling on the island of Culebra, Puerto Rico.',
}

export default function ContactPage() {
  return (
    <>
      <SiteHeader />
      <main>
        <PageHero
          eyebrow="Contact"
          title="Let's begin a quiet conversation."
          intro="Whether you are considering a purchase, a sale, or simply the possibility of island life, we would be glad to hear from you."
          image="/images/coastline.png"
          imageAlt="The Culebra coastline at golden hour"
        />
        <Contact />
      </main>
      <SiteFooter />
    </>
  )
}
