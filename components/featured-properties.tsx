import Image from 'next/image'
import Link from 'next/link'

import type { PropertySummary } from '@/db/properties'
import { Reveal } from '@/components/reveal'
import { formatArea, formatPrice, isLand } from '@/lib/property'

function propertyLocation(property: PropertySummary): string {
  return (
    property.neighborhood ??
    property.location ??
    property.city ??
    'Culebra, Puerto Rico'
  )
}

function propertyDetail(property: PropertySummary): string {
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

type FeaturedPropertiesProps = {
  properties: PropertySummary[]
  limit?: number
}

export function FeaturedProperties({
  properties,
  limit = 3,
}: FeaturedPropertiesProps) {
  const items = properties.slice(0, limit)

  return (
    <section id="properties" className="px-6 py-28 md:px-12 md:py-40">
      <div className="mx-auto max-w-[1600px]">
        <Reveal className="mb-20 md:mb-28">
          <div className="flex flex-col gap-6 border-b border-border pb-10 md:flex-row md:items-end md:justify-between">
            <div>
              <p className="mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent">
                The Collection
              </p>
              <h2 className="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-6xl">
                Residences chosen for their silence.
              </h2>
            </div>
            <p className="max-w-xs text-pretty text-sm font-light leading-relaxed text-muted-foreground">
              Each estate is selected in person, for its light, its outlook, and its
              relationship to the sea.
            </p>
          </div>
        </Reveal>

        {items.length === 0 ? (
          <p className="max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
            The next collection is being prepared.
          </p>
        ) : (
          <div className="flex flex-col gap-28 md:gap-40">
            {items.map((property, index) => {
              const href = `/properties/${property.slug}`

              return (
                <Reveal key={property.id}>
                  <article
                    className={`grid items-center gap-10 md:grid-cols-12 md:gap-16 ${
                      index % 2 === 1 ? 'md:[direction:rtl]' : ''
                    }`}
                  >
                    <div className="md:col-span-8 md:[direction:ltr]">
                      <Link
                        href={href}
                        aria-label={`View ${property.name}`}
                        className="group relative block aspect-[16/10] w-full overflow-hidden"
                      >
                        <Image
                          src={property.heroUrl || '/placeholder.svg'}
                          alt={property.heroAlt}
                          fill
                          unoptimized
                          sizes="(min-width: 768px) 66vw, 100vw"
                          className="object-cover transition-transform duration-[1600ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.04]"
                        />
                      </Link>
                    </div>

                    <div className="md:col-span-4 md:[direction:ltr]">
                      <span className="font-serif text-sm font-light text-accent">
                        ({String(index + 1).padStart(2, '0')})
                      </span>
                      <h3 className="mt-4 font-serif text-3xl font-light leading-tight text-foreground md:text-4xl">
                        <Link
                          href={href}
                          className="transition-colors duration-300 hover:text-accent"
                        >
                          {property.name}
                        </Link>
                      </h3>
                      <p className="mt-3 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                        {propertyLocation(property)}
                      </p>
                      <p className="mt-8 max-w-xs text-sm font-light leading-relaxed text-foreground/80">
                        {propertyDetail(property)}
                      </p>
                      <div className="mt-8 flex items-center justify-between border-t border-border pt-6">
                        <span className="text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                          {formatPrice(property.listPrice)}
                        </span>
                        <a
                          href="#contact"
                          className="group/link inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground"
                        >
                          Enquire
                          <span className="inline-block h-px w-6 bg-foreground transition-all duration-500 group-hover/link:w-10" />
                        </a>
                      </div>
                    </div>
                  </article>
                </Reveal>
              )
            })}
          </div>
        )}
      </div>
    </section>
  )
}
