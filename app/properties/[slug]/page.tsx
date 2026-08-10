import type { Metadata } from 'next'
import Image from 'next/image'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import { BedDouble, Bath, Maximize, Trees, ChevronRight } from 'lucide-react'

import { client } from '@/sanity/lib/client'
import { PROPERTY_BY_SLUG_QUERY } from '@/sanity/lib/queries'
import { urlFor } from '@/sanity/lib/image'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import {
  PropertyTabs,
  type PropertyDetail,
} from '@/components/property/property-tabs'
import { SaveProperty } from '@/components/property/save-property'
import type { GalleryImage } from '@/components/property/property-gallery'
import { formatPrice, isLand } from '@/lib/property'

type PageProps = { params: Promise<{ slug: string }> }

async function getProperty(slug: string): Promise<PropertyDetail | null> {
  return client.fetch(PROPERTY_BY_SLUG_QUERY, { slug }, { cache: 'no-store' })
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { slug } = await params
  const property = await getProperty(slug)
  if (!property) return { title: 'Property — CulebraLuxe' }

  const location = [property.neighborhood, property.city]
    .filter(Boolean)
    .join(', ')
  return {
    title: `${property.title} — CulebraLuxe`,
    description:
      property.shortDescription ??
      `${property.title}${location ? ` in ${location}` : ''}, presented by CulebraLuxe.`,
  }
}

export default async function PropertyPage({ params }: PageProps) {
  const { slug } = await params
  const property = await getProperty(slug)

  if (!property) notFound()

  const land = isLand(property.propertyType)

  const heroUrl = property.heroImage
    ? urlFor(property.heroImage).width(2400).height(1400).quality(85).url()
    : null

  // Build the gallery viewer set: hero first, then gallery items.
  const galleryImages: GalleryImage[] = []
  if (property.heroImage) {
    galleryImages.push({
      url: urlFor(property.heroImage).width(1800).height(1125).url(),
      alt: property.title ?? 'Property',
    })
  }
  for (const g of (property as unknown as {
    gallery?: (GalleryImage & { _key?: string })[]
  }).gallery ?? []) {
    const source = g as unknown as Parameters<typeof urlFor>[0]
    galleryImages.push({
      url: urlFor(source).width(1800).height(1125).url(),
      alt: (g.alt as string) ?? property.title ?? 'Property',
      caption: g.caption ?? null,
    })
  }

  const locationLine = [property.neighborhood, property.city, property.stateOrProvince]
    .filter(Boolean)
    .join(', ')

  const facts: { icon: typeof BedDouble; value: string }[] = []
  if (!land) {
    if (property.bedroomsTotal != null)
      facts.push({ icon: BedDouble, value: `${property.bedroomsTotal} Beds` })
    if (property.bathroomsTotal != null)
      facts.push({ icon: Bath, value: `${property.bathroomsTotal} Baths` })
    if (property.livingArea != null)
      facts.push({
        icon: Maximize,
        value: `${property.livingArea.toLocaleString('en-US')} SF`,
      })
  }
  if (property.lotSizeArea != null) {
    const isAcre = (property.lotSizeUnits ?? '').toLowerCase() === 'acres'
    facts.push({
      icon: Trees,
      value: isAcre
        ? `${property.lotSizeArea} ${property.lotSizeArea === 1 ? 'Acre' : 'Acres'}`
        : `${property.lotSizeArea.toLocaleString('en-US')} SF Lot`,
    })
  }

  const tags = [...(property.viewType ?? [])].slice(0, 5)

  return (
    <>
      <SiteHeader />
      <main>
        {/* Full-bleed cinematic hero (sits behind the transparent header) */}
        <section className="relative h-[78vh] min-h-[540px] w-full overflow-hidden bg-foreground">
          {heroUrl && (
            <Image
              src={heroUrl}
              alt={property.title ?? 'Property'}
              fill
              priority
              sizes="100vw"
              className="object-cover"
            />
          )}
          <div
            aria-hidden
            className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-black/30"
          />

          {property.featured && (
            <span className="absolute left-6 top-28 bg-background/90 px-4 py-1.5 text-[11px] font-light uppercase tracking-[0.22em] text-foreground backdrop-blur-sm md:left-12">
              Featured
            </span>
          )}

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
                {facts.map((f) => (
                  <span
                    key={f.value}
                    className="flex items-center gap-2 text-xs font-light uppercase tracking-[0.14em] text-background/85"
                  >
                    <f.icon className="h-4 w-4" aria-hidden />
                    {f.value}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </section>

        <div className="mx-auto max-w-[1600px] px-6 py-12 md:px-12 md:py-16">
          {/* Breadcrumb — "Properties" is plain text until the buyers grid exists */}
          <nav
            aria-label="Breadcrumb"
            className="mb-12 flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.18em] text-muted-foreground"
          >
            <Link href="/" className="transition-colors hover:text-foreground">
              Home
            </Link>
            <ChevronRight className="h-3 w-3" aria-hidden />
            <span>Properties</span>
            <ChevronRight className="h-3 w-3" aria-hidden />
            <span className="text-foreground">{property.title}</span>
          </nav>

          {/* Identity actions: view tags + inquire / save */}
          <div className="mb-14 flex flex-col gap-8 border-b border-border pb-14 md:flex-row md:items-end md:justify-between">
            <div className="flex flex-wrap gap-3">
              {tags.map((t) => (
                <span
                  key={t}
                  className="border border-border px-4 py-2 text-xs font-light uppercase tracking-[0.16em] text-foreground/80"
                >
                  {t}
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
              <SaveProperty propertyId={property._id} />
            </div>
          </div>

          <PropertyTabs property={property} galleryImages={galleryImages} />
        </div>
      </main>
      <SiteFooter />
    </>
  )
}
