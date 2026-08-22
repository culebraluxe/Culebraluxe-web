'use client'

import Image from 'next/image'
import Link from 'next/link'
import { useEffect, useState } from 'react'
import { Heart } from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import {
  FAVORITES_CHANGED_EVENT,
  pruneFavorites,
  removeFavorite,
} from '@/lib/favorites'
import { formatArea, formatPrice, isLand } from '@/lib/property'

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

type FavoritesViewProps = {
  properties: PropertySummary[]
}

export function FavoritesView({ properties }: FavoritesViewProps) {
  const [mounted, setMounted] = useState(false)
  const [items, setItems] = useState<PropertySummary[]>([])

  useEffect(() => {
    setMounted(true)

    const byId = new Map(properties.map((property) => [property.id, property]))
    const bySlug = new Map(properties.map((property) => [property.slug, property]))

    const refresh = () => {
      // Drop saved entries that no longer correspond to a live public
      // listing (canonical id or slug) so delisted properties cannot
      // permanently occupy the saved set.
      const pruned = pruneFavorites(
        properties.map((property) => property.id),
        properties.map((property) => property.slug),
      )

      const matched: PropertySummary[] = []
      const seen = new Set<string>()

      for (const entry of pruned) {
        const property = byId.get(entry.id) ?? bySlug.get(entry.slug)
        if (property && !seen.has(property.id)) {
          seen.add(property.id)
          matched.push(property)
        }
      }

      setItems(matched)
    }

    refresh()
    window.addEventListener(FAVORITES_CHANGED_EVENT, refresh)
    return () => window.removeEventListener(FAVORITES_CHANGED_EVENT, refresh)
  }, [properties])

  if (!mounted) {
    return null
  }

  return (
    <section className="px-6 py-16 md:px-12 md:py-20">
      <div className="mx-auto max-w-[1600px]">
        <p className="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">
          Saved
        </p>
        <h1 className="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
          Saved properties
        </h1>
        <p className="mt-4 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
          Properties you have saved on this device for a closer look.
        </p>

        {items.length === 0 ? (
          <div className="mt-12 border-t border-border pt-12">
            <p className="font-serif text-2xl font-light text-foreground">
              Nothing saved yet.
            </p>
            <p className="mt-3 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
              Tap the heart on any property to keep it here for later.
            </p>
            <Link
              href="/buyers"
              className="mt-6 inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors hover:text-accent"
            >
              Explore properties
              <span aria-hidden>→</span>
            </Link>
          </div>
        ) : (
          <div className="mt-12 grid gap-x-7 gap-y-14 md:grid-cols-2 xl:grid-cols-3">
            {items.map((property) => {
              const href = `/properties/${property.slug}`

              return (
                <article key={property.id} className="group relative">
                  <div className="relative aspect-[4/3] overflow-hidden bg-muted">
                    {property.heroUrl ? (
                      <Image
                        src={property.heroUrl}
                        alt={property.heroAlt}
                        fill
                        unoptimized
                        sizes="(min-width: 1024px) 33vw, 100vw"
                        className="object-cover transition-transform duration-[1600ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.025]"
                      />
                    ) : (
                      <div className="h-full w-full bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]" />
                    )}

                    <button
                      type="button"
                      onClick={() => removeFavorite(property.id)}
                      aria-label={`Remove ${property.name} from saved`}
                      className="absolute right-4 top-4 z-30 flex h-10 w-10 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background"
                    >
                      <Heart className="h-4 w-4 fill-accent text-accent" />
                    </button>
                  </div>

                  <Link
                    href={href}
                    aria-label={`View ${property.name}`}
                    className="absolute inset-0 z-10"
                  />

                  <div className="pointer-events-none relative z-20 pt-5">
                    <p className="mb-2 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                      {property.neighborhood ??
                        property.location ??
                        property.city ??
                        'Culebra, Puerto Rico'}
                    </p>
                    <div className="flex items-start justify-between gap-6">
                      <h3 className="font-serif text-2xl font-light leading-tight text-foreground">
                        {property.name}
                      </h3>
                      <p className="whitespace-nowrap pt-1 text-sm font-light text-foreground">
                        {formatPrice(property.listPrice)}
                      </p>
                    </div>
                    <p className="mt-3 text-[11px] font-light uppercase tracking-[0.14em] text-muted-foreground">
                      {propertyFacts(property)}
                    </p>
                  </div>
                </article>
              )
            })}
          </div>
        )}
      </div>
    </section>
  )
}
