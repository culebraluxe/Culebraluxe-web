import Image from 'next/image'
import Link from 'next/link'

import { getSimilarProperties } from '@/db/properties'
import type { PropertySummary } from '@/db/properties'
import { formatArea, formatPrice, isLand } from '@/lib/property'

function propertyLocation(property: PropertySummary): string {
  return (
    property.neighborhood ??
    property.location ??
    property.city ??
    'Culebra, Puerto Rico'
  )
}

function propertyFacts(property: PropertySummary): string {
  const parts: string[] = []

  if (isLand(property.propertyType)) {
    const lot = formatArea(property.lotSize, property.lotSizeUnits)
    if (lot) parts.push(lot)
  } else {
    if (property.bedrooms != null) parts.push(`${property.bedrooms} Bed`)
    if (property.bathrooms != null) parts.push(`${property.bathrooms} Bath`)
    if (property.squareFeet != null) {
      parts.push(`${property.squareFeet.toLocaleString('en-US')} SF`)
    }
  }

  if (property.views[0]) {
    parts.push(`${property.views[0]} View`)
  }

  return parts.join(' · ')
}

type SimilarPropertiesProps = {
  propertyId: string
  propertyType: string | null | undefined
  city: string | null | undefined
  neighborhood: string | null | undefined
  listPrice: number | null | undefined
}

export async function SimilarProperties({
  propertyId,
  propertyType,
  city,
  neighborhood,
  listPrice,
}: SimilarPropertiesProps) {
  const similar = await getSimilarProperties(propertyId, {
    propertyType: propertyType ?? null,
    city: city ?? null,
    neighborhood: neighborhood ?? null,
    listPrice: listPrice ?? null,
  })

  if (similar.length === 0) {
    return null
  }

  return (
    <section className="mt-24 md:mt-32">
      <div className="mb-12 flex flex-col gap-6 border-b border-border pb-10 md:flex-row md:items-end md:justify-between">
        <div>
          <p className="mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent">
            Continue Exploring
          </p>
          <h2 className="text-balance font-serif text-3xl font-light leading-[1.08] text-foreground md:text-5xl">
            Similar Residences
          </h2>
        </div>
        <Link
          href="/buyers"
          className="group inline-flex shrink-0 items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors hover:text-accent"
        >
          View all properties
          <span className="inline-block h-px w-6 bg-foreground transition-all duration-500 group-hover:w-10" />
        </Link>
      </div>

      <div className="grid gap-8 md:grid-cols-3 md:gap-6">
        {similar.map((property) => {
          const href = `/properties/${property.slug}`

          return (
            <article key={property.id} className="group">
              <Link
                href={href}
                aria-label={`View ${property.name}`}
                className="relative block aspect-[16/10] w-full overflow-hidden bg-muted"
              >
                <Image
                  src={property.heroUrl || '/placeholder.svg'}
                  alt={property.heroAlt}
                  fill
                  unoptimized
                  sizes="(min-width: 768px) 33vw, 100vw"
                  className="object-cover transition-transform duration-[1400ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.035]"
                />
              </Link>

              <div className="mt-5 border-t border-border pt-4">
                <h3 className="font-serif text-xl font-light leading-tight text-foreground md:text-2xl">
                  <Link
                    href={href}
                    className="transition-colors duration-300 hover:text-accent"
                  >
                    {property.name}
                  </Link>
                </h3>
                <p className="mt-2 text-[10px] font-light uppercase tracking-[0.24em] text-muted-foreground">
                  {propertyLocation(property)}
                </p>
                <p className="mt-3 text-xs font-light text-muted-foreground">
                  {propertyFacts(property)}
                </p>
                <p className="mt-4 text-sm font-light text-foreground">
                  {formatPrice(property.listPrice)}
                </p>
              </div>
            </article>
          )
        })}
      </div>
    </section>
  )
}
