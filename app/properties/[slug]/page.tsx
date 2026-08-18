import type { Metadata } from 'next'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ChevronRight } from 'lucide-react'

import { getPropertyBySlug } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PropertyCockpit } from '@/components/property/property-cockpit'
import { PropertyTabs } from '@/components/property/property-tabs'

type PageProps = {
  params: Promise<{ slug: string }>
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { slug } = await params
  const result = await getPropertyBySlug(slug)

  if (!result) {
    return {
      title: 'Property — CulebraLuxe',
    }
  }

  const { property } = result

  const location = [
    property.neighborhood,
    property.city,
  ]
    .filter(Boolean)
    .join(', ')

  return {
    title: `${property.title} — CulebraLuxe`,
    description:
      property.shortDescription ??
      `${property.title}${
        location ? ` in ${location}` : ''
      }, presented by CulebraLuxe.`,
  }
}

export default async function PropertyPage({
  params,
}: PageProps) {
  const { slug } = await params

  const result = await getPropertyBySlug(slug)

  if (!result) {
    notFound()
  }

  const {
    property,
    heroUrl,
    galleryImages,
    videos,
    documents,
  } = result
  const googleMapsApiKey =
    process.env.NODE_ENV === 'production'
      ? process.env.GOOGLE_MAPS_API_KEY?.trim() || null
      : process.env.GOOGLE_MAPS_DEMO_KEY?.trim() ||
        process.env.GOOGLE_MAPS_API_KEY?.trim() ||
        null

  return (
    <>
      <SiteHeader />

      <main>
        <div className="mx-auto max-w-[1600px] px-6 py-8 md:px-12 md:py-10">
          <nav
            aria-label="Breadcrumb"
            className="mb-6 flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.18em] text-muted-foreground"
          >
            <Link
              href="/"
              className="transition-colors hover:text-foreground"
            >
              Home
            </Link>

            <ChevronRight
              className="h-3 w-3"
              aria-hidden
            />

            <Link
              href="/buyers"
              className="transition-colors hover:text-foreground"
            >
              Properties
            </Link>

            <ChevronRight
              className="h-3 w-3"
              aria-hidden
            />

            <span className="text-foreground">
              {property.title}
            </span>
          </nav>

          <PropertyCockpit
            property={property}
            heroUrl={heroUrl}
            galleryImages={galleryImages}
          />

          <PropertyTabs
            property={property}
            videos={videos}
            documents={documents}
            googleMapsApiKey={googleMapsApiKey}
          />
        </div>
      </main>

      <SiteFooter />
    </>
  )
}
