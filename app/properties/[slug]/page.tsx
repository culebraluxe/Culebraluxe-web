import type { Metadata } from 'next'
import Image from 'next/image'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import {
  BedDouble,
  Bath,
  Maximize,
  Trees,
  ChevronRight,
} from 'lucide-react'

import { getPropertyBySlug } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PropertyTabs } from '@/components/property/property-tabs'
import { SaveProperty } from '@/components/property/save-property'
import { formatPrice, isLand } from '@/lib/property'

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
  } = result

  const land = isLand(property.propertyType)

  const locationLine = [
    property.neighborhood,
    property.city,
    property.stateOrProvince,
  ]
    .filter(Boolean)
    .join(', ')

  const facts: {
    icon: typeof BedDouble
    value: string
  }[] = []

  if (!land) {
    if (property.bedroomsTotal != null) {
      facts.push({
        icon: BedDouble,
        value: `${property.bedroomsTotal} Beds`,
      })
    }

    if (property.bathroomsTotal != null) {
      facts.push({
        icon: Bath,
        value: `${property.bathroomsTotal} Baths`,
      })
    }

    if (property.livingArea != null) {
      facts.push({
        icon: Maximize,
        value: `${property.livingArea.toLocaleString('en-US')} SF`,
      })
    }
  }

  if (property.lotSizeArea != null) {
    const isAcre =
      (property.lotSizeUnits ?? '').toLowerCase() === 'acres'

    facts.push({
      icon: Trees,
      value: isAcre
        ? `${property.lotSizeArea} ${
            property.lotSizeArea === 1 ? 'Acre' : 'Acres'
          }`
        : `${property.lotSizeArea.toLocaleString('en-US')} SF Lot`,
    })
  }

  const tags = [
    ...(property.viewType ?? []),
  ].slice(0, 5)

  return (
    <>
      <SiteHeader />

      <main>
        <section className="relative h-[78vh] min-h-[540px] w-full overflow-hidden bg-foreground">
          {heroUrl && (
            <Image
              src={heroUrl}
              alt={property.title ?? 'Property'}
              fill
              priority
              unoptimized
              sizes="100vw"
              className="object-cover"
            />
          )}

          <div
            aria-hidden
            className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-black/30"
          />

          <div className="absolute inset-x-0 bottom-0">
            <div className="mx-auto max-w-[1600px] px-6 pb-12 md:px-12 md:pb-16">
              {locationLine && (
                <p className="mb-4 text-xs font-light uppercase tracking-[0.28em] text-background/80">
                  {locationLine}
                </p>
              )}

              <h1 className="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.02] text-background md:text-6xl lg:text-7xl">
                {property.title}
              </h1>

              <div className="mt-6 flex flex-wrap items-center gap-x-6 gap-y-3 border-t border-background/20 pt-6">
                <span className="text-lg font-light text-background md:text-xl">
                  {formatPrice(property.listPrice)}
                </span>

                {facts.map((fact) => (
                  <span
                    key={fact.value}
                    className="flex items-center gap-2 text-xs font-light uppercase tracking-[0.14em] text-background/85"
                  >
                    <fact.icon
                      className="h-4 w-4"
                      aria-hidden
                    />

                    {fact.value}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </section>

        <div className="mx-auto max-w-[1600px] px-6 py-12 md:px-12 md:py-16">
          <nav
            aria-label="Breadcrumb"
            className="mb-12 flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.18em] text-muted-foreground"
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

          <div className="mb-14 flex flex-col gap-8 border-b border-border pb-14 md:flex-row md:items-end md:justify-between">
            <div className="flex flex-wrap gap-3">
              {tags.map((tag) => (
                <span
                  key={tag}
                  className="border border-border px-4 py-2 text-xs font-light uppercase tracking-[0.16em] text-foreground/80"
                >
                  {tag}
                </span>
              ))}
            </div>

            <div className="flex flex-wrap items-stretch gap-3">
              <Link
                href="/#contact"
                className="inline-flex items-center justify-center bg-accent px-10 py-4 text-xs font-light uppercase tracking-[0.2em] text-accent-foreground transition-opacity duration-500 hover:opacity-90"
              >
                Inquire About This Property
              </Link>

              <SaveProperty
                propertyId={property._id}
              />
            </div>
          </div>

          <PropertyTabs
            property={property}
            galleryImages={galleryImages}
            videos={videos}
          />
        </div>
      </main>

      <SiteFooter />
    </>
  )
}