import type { Metadata } from 'next'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Contact } from '@/components/contact'
import { getPropertyIntroById } from '@/db/properties'
import { getMarketingContent } from '@/db/marketing-content'
import { buildContactPageContent } from '@/lib/marketing-content'
import { normalizeServiceKey } from '@/lib/services'

export const metadata: Metadata = {
  title: 'Contact — CulebraLuxe',
  description:
    'Begin a quiet conversation with CulebraLuxe about buying or selling on the island of Culebra, Puerto Rico.',
}

export const dynamic = 'force-dynamic'

type ContactPageProps = {
  searchParams: Promise<{
    propertyId?: string
    requestType?: string
    service?: string
  }>
}

export default async function ContactPage({ searchParams }: ContactPageProps) {
  const query = await searchParams
  const requestType: 'private_viewing' | 'property_information' | undefined =
    query.requestType === 'private_viewing' ||
    query.requestType === 'property_information'
      ? query.requestType
      : undefined
  // Service intent from a service-specific CTA (`/contact?service=...`). It is
  // allow-listed against the supported services vocabulary so an arbitrary
  // query value is dropped rather than persisted. It only applies to the
  // property-less general enquiry path; property-scoped requests ignore it.
  const service = normalizeServiceKey(
    typeof query.service === 'string' ? query.service : undefined,
  )
  let propertyName: string | null = null
  if (query.propertyId) {
    const introResult = await getPropertyIntroById(query.propertyId)
    propertyName = introResult.ok ? introResult.data?.name ?? null : null
  }
  const propertyContext = requestType
    ? {
        propertyId: query.propertyId,
        requestType,
        propertyName,
      }
    : undefined

  const contentResult = await getMarketingContent()
  const page = buildContactPageContent(contentResult.ok ? contentResult.data : [])

  return (
    <>
      <SiteHeader />
      <main>
        {page.hero ? (
          <PageHero
            eyebrow={page.hero.eyebrow ?? ''}
            title={page.hero.title ?? ''}
            intro={page.hero.body ?? undefined}
            image={page.hero.imagePath ?? '/images/coastline.png'}
            imageAlt={page.hero.imageAlt ?? 'The Culebra coastline at golden hour'}
          />
        ) : null}
        {page.contact ? (
          <Contact
            content={page.contact}
            propertyContext={propertyContext}
            service={service}
          />
        ) : null}
      </main>
      <SiteFooter />
    </>
  )
}
