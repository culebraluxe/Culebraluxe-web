import type { Metadata } from 'next'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Contact } from '@/components/contact'
import { getPropertyIntroById } from '@/db/properties'

export const metadata: Metadata = {
  title: 'Contact — CulebraLuxe',
  description:
    'Begin a quiet conversation with CulebraLuxe about buying or selling on the island of Culebra, Puerto Rico.',
}

type ContactPageProps = {
  searchParams: Promise<{
    propertyId?: string
    requestType?: string
  }>
}

export default async function ContactPage({ searchParams }: ContactPageProps) {
  const query = await searchParams
  const requestType: 'private_viewing' | 'property_information' | undefined =
    query.requestType === 'private_viewing' ||
    query.requestType === 'property_information'
      ? query.requestType
      : undefined
  const propertyContext = requestType
    ? {
        propertyId: query.propertyId,
        requestType,
        propertyName: query.propertyId
          ? (await getPropertyIntroById(query.propertyId))?.name ?? null
          : null,
      }
    : undefined

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
        <Contact propertyContext={propertyContext} />
      </main>
      <SiteFooter />
    </>
  )
}
