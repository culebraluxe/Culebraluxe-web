import type { Metadata } from 'next'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Contact } from '@/components/contact'
import { getPropertyIntroById } from '@/db/properties'
import { getMarketingContent } from '@/db/marketing-content'
import { buildContactPageContent } from '@/lib/marketing-content'

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

  const page = buildContactPageContent(await getMarketingContent())

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
          <Contact content={page.contact} propertyContext={propertyContext} />
        ) : null}
      </main>
      <SiteFooter />
    </>
  )
}
