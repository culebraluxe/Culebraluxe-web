import Image from 'next/image'
import Link from 'next/link'
import { ArrowRight } from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import { SaveProperty } from '@/components/property/save-property'
import { formatArea, formatPrice, isLand } from '@/lib/property'

function cardFacts(p: PropertySummary): string {
  const parts: string[] = []
  if (!isLand(p.propertyType)) {
    if (p.bedrooms != null) parts.push(`${p.bedrooms} Bed`)
    if (p.bathrooms != null) parts.push(`${p.bathrooms} Bath`)
  } else {
    const lot = formatArea(p.lotSize, p.lotSizeUnits)
    if (lot) parts.push(lot)
  }
  if (p.views[0]) parts.push(`${p.views[0]} View`)
  return parts.join('  ·  ')
}

type HomePropertiesProps = {
  properties: PropertySummary[]
  eyebrow?: string
  title?: string
  intro?: string
  /** Number of cards to show. Defaults to 4. */
  limit?: number
  /** Optional "view all" call-to-action. Omit to hide it. */
  cta?: { href: string; label: string } | null
}

export function HomeProperties({
  properties,
  eyebrow = 'The Portfolio',
  title = 'Find your place in Culebra.',
  intro = 'Exquisite properties on an extraordinary island — each chosen for its light, its outlook, and its relationship to the sea.',
  limit = 4,
  cta = { href: '/#properties', label: 'View All Properties' },
}: HomePropertiesProps) {
  const items = properties.slice(0, limit)
  if (items.length === 0) return null

  return (
    <section className="bg-foreground px-6 py-24 text-background md:px-12 md:py-32">
      <div className="mx-auto max-w-[1600px]">
        <div className="mb-14 flex flex-col gap-8 md:flex-row md:items-end md:justify-between">
          <div className="max-w-xl">
            <p className="mb-4 text-xs font-light uppercase tracking-[0.34em] text-background/60">
              {eyebrow}
            </p>
            <h2 className="text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl">
              {title}
            </h2>
            <p className="mt-5 max-w-md text-pretty text-sm font-light leading-relaxed text-background/70">
              {intro}
            </p>
          </div>
          {cta && (
            <Link
              href={cta.href}
              className="group inline-flex items-center gap-3 self-start border border-background/30 px-8 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 hover:border-background md:self-auto"
            >
              {cta.label}
              <ArrowRight className="h-4 w-4 transition-transform duration-500 group-hover:translate-x-1" />
            </Link>
          )}
        </div>

        <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
          {items.map((p) => {
            const href = `/properties/${p.slug}`

            return (
              <article key={p.id} className="group flex flex-col">
                <div className="relative aspect-[5/4] w-full overflow-hidden bg-background/10">
                  <Link href={href} aria-label={p.name}>
                    {p.heroUrl && (
                      <Image
                        src={p.heroUrl}
                        alt={p.name}
                        fill
                        unoptimized
                        sizes="(min-width: 1024px) 22vw, (min-width: 640px) 45vw, 90vw"
                        className="object-cover transition-transform duration-[1400ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.05]"
                      />
                    )}
                  </Link>
                  {p.featured && (
                    <span className="absolute left-3 top-3 bg-background/90 px-3 py-1 text-[10px] font-light uppercase tracking-[0.18em] text-foreground">
                      Featured
                    </span>
                  )}
                  <SaveProperty
                    propertyId={p.id}
                    slug={p.slug}
                    name={p.name}
                    variant="icon"
                    className="absolute right-3 top-3"
                  />
                </div>
                <div className="mt-5 flex items-baseline justify-between gap-4">
                  <Link
                    href={href}
                    className="font-serif text-xl font-light transition-colors duration-300 hover:text-background/70"
                  >
                    {p.name}
                  </Link>
                  <span className="whitespace-nowrap text-sm font-light text-background/80">
                    {formatPrice(p.listPrice)}
                  </span>
                </div>
                <p className="mt-2 text-[11px] font-light uppercase tracking-[0.16em] text-background/55">
                  {cardFacts(p)}
                </p>
              </article>
            )
          })}
        </div>
      </div>
    </section>
  )
}
