import type { Metadata } from 'next'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ChevronRight } from 'lucide-react'

import { getPropertyBySlug, getPublicPropertySlugs } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PropertyCockpit } from '@/components/property/property-cockpit'
import { PropertyTabs } from '@/components/property/property-tabs'
import { RecentlyViewed } from '@/components/property/recently-viewed'
import { SimilarProperties } from '@/components/property/similar-properties'

type PageProps = {
  params: Promise<{ slug: string }>
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { slug } = await params
  const result = await getPropertyBySlug(slug)

  if (!result.ok || result.data === null) {
    return {
      title: 'Property — CulebraLuxe',
    }
  }

  const { property } = result.data

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

  // DB failure -> controlled "Property temporarily unavailable" (NOT a 404).
  if (!result.ok) {
    return (
      <>
        <SiteHeader />
        <main className="flex min-h-[70svh] items-center bg-[#f5f2ec] px-6 md:px-12">
          <div className="mx-auto w-full max-w-2xl">
            <h1 className="font-serif text-4xl font-light leading-[1.05] text-[#030f23]">
              This property is temporarily unavailable.
            </h1>
            <p className="mt-6 max-w-xl text-sm font-light leading-relaxed text-[#030f23]/60">
              We could not load this property right now. Please try again in a moment.
            </p>
          </div>
        </main>
        <SiteFooter />
      </>
    )
  }

  // Successful query with zero matching rows -> genuine 404.
  if (result.data === null) {
    notFound()
  }

  const {
    property,
    heroUrl,
    galleryImages,
    videos,
    documents,
  } = result.data
  const googleMapsApiKey =
    process.env.NODE_ENV === 'production'
      ? process.env.GOOGLE_MAPS_API_KEY?.trim() || null
      : process.env.GOOGLE_MAPS_DEMO_KEY?.trim() ||
        process.env.GOOGLE_MAPS_API_KEY?.trim() ||
        null

  const publicSlugsResult = await getPublicPropertySlugs()
  const publicSlugs = publicSlugsResult.ok ? publicSlugsResult.data : []

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

          <SimilarProperties
            propertyId={property._id}
            propertyType={property.propertyType}
            city={property.city}
            neighborhood={property.neighborhood}
            listPrice={property.listPrice}
          />

          <RecentlyViewed
            slug={slug}
            id={property._id}
            name={property.title ?? slug}
            validSlugs={publicSlugs}
          />
        </div>
      </main>

      <SiteFooter />
    </>
  )
}
