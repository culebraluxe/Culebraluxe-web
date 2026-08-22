'use client'

import Image from 'next/image'
import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import { usePathname, useRouter, useSearchParams } from 'next/navigation'
import {
  ArrowUpRight,
  Search,
} from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import { SaveProperty } from '@/components/property/save-property'
import { CompareProperty } from '@/components/property/compare-property'
import { FeaturedPropertyCarousel } from '@/components/property/featured-property-carousel'
import { SavedSearchesPanel } from '@/components/property/saved-searches-panel'
import {
  formatArea,
  formatPrice,
  isLand,
} from '@/lib/property'
import {
  applySearchFilters,
  searchFiltersToQuery,
  searchParamsToFilters,
} from '@/lib/search-contract'
import type {
  SearchCategory,
  SearchFilters,
  SearchSort,
} from '@/lib/search-contract'

type BuyersPropertyShowroomProps = {
  properties: PropertySummary[]
  featured: PropertySummary[]
  viewOptions: string[]
  initial: SearchFilters
}

function propertyLocation(property: PropertySummary) {
  return (
    property.neighborhood ??
    property.location ??
    property.city ??
    'Culebra, Puerto Rico'
  )
}

function propertyFacts(property: PropertySummary) {
  const parts: string[] = []

  if (isLand(property.propertyType)) {
    const lot = formatArea(
      property.lotSize,
      property.lotSizeUnits,
    )

    if (lot) {
      parts.push(lot)
    }
  } else {
    if (property.bedrooms != null) {
      parts.push(`${property.bedrooms} Bed`)
    }

    if (property.bathrooms != null) {
      parts.push(`${property.bathrooms} Bath`)
    }

    if (property.squareFeet != null) {
      parts.push(
        `${property.squareFeet.toLocaleString('en-US')} SF`,
      )
    }
  }

  if (property.views[0]) {
    parts.push(`${property.views[0]} View`)
  }

  return parts.join('  ·  ')
}

function PropertyImage({
  property,
}: {
  property: PropertySummary
}) {
  if (!property.heroUrl) {
    return (
      <div className="h-full w-full bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]" />
    )
  }

  return (
    <Image
      src={property.heroUrl}
      alt={property.heroAlt}
      fill
      unoptimized
      sizes="(min-width: 1024px) 80vw, 100vw"
      className="object-cover transition-transform duration-[1600ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.025]"
    />
  )
}

function InventoryCard({
  property,
}: {
  property: PropertySummary
}) {
  const href = `/properties/${property.slug}`

  return (
    <article className="group relative">
      <div className="relative aspect-[4/3] overflow-hidden bg-muted">
        <PropertyImage property={property} />

        {property.featured && (
          <span className="absolute left-4 top-4 z-20 bg-background/90 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.18em] text-foreground backdrop-blur-sm">
            Featured
          </span>
        )}

        {isLand(property.propertyType) && (
          <span className="absolute bottom-4 left-4 z-20 bg-foreground/80 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.18em] text-background backdrop-blur-sm">
            Land
          </span>
        )}

        <SaveProperty
          propertyId={property.id}
          slug={property.slug}
          name={property.name}
          variant="icon"
          className="absolute right-4 top-4 z-30"
        />

        <CompareProperty
          id={property.id}
          slug={property.slug}
          name={property.name}
          className="absolute right-14 top-4 z-30"
        />
      </div>

      <Link
        href={href}
        aria-label={`View ${property.name}`}
        className="absolute inset-0 z-10"
      />

      <div className="pointer-events-none relative z-20 pt-5">
        <p className="mb-2 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
          {propertyLocation(property)}
        </p>

        <div className="flex items-start justify-between gap-6">
          <h3 className="font-serif text-2xl font-light leading-tight text-foreground">
            {property.name}
          </h3>

          <p className="whitespace-nowrap pt-1 text-sm font-light text-foreground">
            {formatPrice(property.listPrice)}
          </p>
        </div>

        <div className="mt-3 flex items-center justify-between gap-4">
          <p className="text-[11px] font-light uppercase tracking-[0.14em] text-muted-foreground">
            {propertyFacts(property)}
          </p>

          <ArrowUpRight className="h-4 w-4 flex-none text-muted-foreground transition-transform duration-500 group-hover:-translate-y-0.5 group-hover:translate-x-0.5" />
        </div>
      </div>
    </article>
  )
}

export function BuyersPropertyShowroom({
  properties,
  featured,
  viewOptions,
  initial,
}: BuyersPropertyShowroomProps) {
  const router = useRouter()
  const pathname = usePathname()
  const searchParams = useSearchParams()

  const [category, setCategory] =
    useState<SearchCategory>(initial.category)

  const [search, setSearch] = useState(initial.q)
  const [maxPrice, setMaxPrice] = useState(initial.maxPrice)
  const [beds, setBeds] = useState(initial.beds)
  const [view, setView] = useState(initial.view)
  const [sort, setSort] =
    useState<SearchSort>(initial.sort)

  // PX-24C: URL is the source of truth. Reconcile local controls from the URL
  // whenever it changes (including back/forward navigation) without pushing.
  useEffect(() => {
    const next = searchParamsToFilters(searchParams)
    setCategory(next.category)
    setSearch(next.q)
    setMaxPrice(next.maxPrice)
    setBeds(next.beds)
    setView(next.view)
    setSort(next.sort)
  }, [searchParams])

  // Push local filter state into the URL. Uses replace so typing/controls do
  // not spam history, and skips when the URL already matches to avoid churn on
  // back/forward reconciliation.
  useEffect(() => {
    const qs = searchFiltersToQuery({
      category,
      q: search,
      maxPrice,
      beds,
      view,
      sort,
    })
    if (qs === searchParams.toString()) return
    router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false })
  }, [category, search, maxPrice, beds, view, sort, router, pathname, searchParams])

  const showroom = featured

  // Canonical PX-24 pipeline: match on the same contract as the saved-search
  // matcher, then order canonically. Applied over the server-filtered list it
  // is idempotent — it only keeps the list responsive between round trips.
  const filtered = useMemo(
    () =>
      applySearchFilters(properties, {
        category,
        q: search,
        maxPrice,
        beds,
        view,
        sort,
      }),
    [properties, category, search, maxPrice, beds, view, sort],
  )

  if (properties.length === 0) {
    return (
      <section className="px-6 py-24 md:px-12 md:py-32">
        <div className="mx-auto max-w-[1600px]">
          <p className="font-serif text-3xl font-light text-foreground">
            New opportunities are being prepared.
          </p>

          <p className="mt-4 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
            Contact CulebraLuxe for private and upcoming
            properties on the island.
          </p>
        </div>
      </section>
    )
  }

  return (
    <>
      <section className="px-6 pb-12 pt-12 md:px-12 md:pb-16 md:pt-16">
        <div className="mx-auto max-w-[1600px]">
          <div className="mb-6 flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
            <div>
              <p className="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">
                Selected Properties
              </p>

              <h2 className="max-w-3xl text-balance font-serif text-4xl font-light leading-[1.03] text-foreground md:text-5xl">
                Exceptional places.
                <br />
                Singular settings.
              </h2>
            </div>

            <p className="max-w-md text-sm font-light leading-relaxed text-muted-foreground md:pb-1">
              A considered selection of residences and
              land across Culebra.
            </p>
          </div>

          <div className="h-[320px] w-full overflow-hidden sm:h-[360px] md:h-[400px] lg:h-[420px]">
            <FeaturedPropertyCarousel
              properties={showroom}
            />
          </div>
        </div>
      </section>

      <section
        id="inventory"
        className="border-t border-border bg-background px-6 py-16 md:px-12 md:py-20"
      >
        <div className="mx-auto max-w-[1600px]">
          <div className="mb-10">
            <p className="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">
              Explore Culebra
            </p>

            <div className="flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
              <h2 className="font-serif text-4xl font-light leading-none text-foreground md:text-5xl">
                Available properties.
              </h2>

              <div className="flex items-center gap-5">
                <p className="text-xs font-light uppercase tracking-[0.18em] text-muted-foreground">
                  {filtered.length}{' '}
                  {filtered.length === 1
                    ? 'property'
                    : 'properties'}
                </p>
                <Link
                  href="/favorites"
                  className="text-xs font-light uppercase tracking-[0.18em] text-accent transition-colors hover:text-foreground"
                >
                  Saved
                </Link>
              </div>
            </div>
          </div>

          <div className="mb-7 flex flex-wrap gap-x-8 gap-y-3 border-b border-border">
            {(
              [
                ['all', 'All'],
                ['homes', 'Homes & Villas'],
                ['land', 'Land'],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                type="button"
                onClick={() =>
                  setCategory(value)
                }
                className={`relative -mb-px pb-4 text-xs font-light uppercase tracking-[0.2em] transition-colors ${
                  category === value
                    ? 'text-foreground'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                {label}

                <span
                  className={`absolute inset-x-0 bottom-0 h-px ${
                    category === value
                      ? 'bg-foreground'
                      : 'bg-transparent'
                  }`}
                />
              </button>
            ))}
          </div>

          <div className="sticky top-0 z-30 mb-12 border-y border-border bg-background/95 py-4 backdrop-blur-md">
            <div className="grid gap-3 md:grid-cols-12">
              <label className="relative md:col-span-4">
                <span className="sr-only">
                  Search properties
                </span>

                <Search className="absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />

                <input
                  value={search}
                  onChange={(event) =>
                    setSearch(event.target.value)
                  }
                  placeholder="Property, neighborhood, view..."
                  className="h-12 w-full border border-border bg-transparent pl-11 pr-4 text-sm font-light text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-foreground"
                />
              </label>

              <select
                value={maxPrice}
                onChange={(event) =>
                  setMaxPrice(event.target.value)
                }
                className="h-12 border border-border bg-background px-4 text-xs font-light uppercase tracking-[0.12em] text-foreground outline-none md:col-span-2"
              >
                <option value="">Any Price</option>
                <option value="1000000">
                  Up to $1M
                </option>
                <option value="2000000">
                  Up to $2M
                </option>
                <option value="3000000">
                  Up to $3M
                </option>
                <option value="5000000">
                  Up to $5M
                </option>
                <option value="10000000">
                  Up to $10M
                </option>
              </select>

              <select
                value={beds}
                onChange={(event) =>
                  setBeds(event.target.value)
                }
                disabled={category === 'land'}
                className="h-12 border border-border bg-background px-4 text-xs font-light uppercase tracking-[0.12em] text-foreground outline-none disabled:opacity-40 md:col-span-2"
              >
                <option value="">Any Beds</option>
                <option value="2">2+ Beds</option>
                <option value="3">3+ Beds</option>
                <option value="4">4+ Beds</option>
                <option value="5">5+ Beds</option>
              </select>

              <select
                value={view}
                onChange={(event) =>
                  setView(event.target.value)
                }
                className="h-12 border border-border bg-background px-4 text-xs font-light uppercase tracking-[0.12em] text-foreground outline-none md:col-span-2"
              >
                <option value="">Any View</option>

                {viewOptions.map((option) => (
                  <option
                    key={option}
                    value={option}
                  >
                    {option}
                  </option>
                ))}
              </select>

              <select
                value={sort}
                onChange={(event) =>
                  setSort(
                    event.target
                      .value as SearchSort,
                  )
                }
                className="h-12 border border-border bg-background px-4 text-xs font-light uppercase tracking-[0.12em] text-foreground outline-none md:col-span-2"
              >
                <option value="featured">
                  Featured
                </option>
                <option value="price-high">
                  Price High
                </option>
                <option value="price-low">
                  Price Low
                </option>
                <option value="name">
                  Name
                </option>
              </select>
            </div>
          </div>

          {/* PX-23 Saved Searches + Alerts: save the current filter state and
              surface "new matches" alerts against the live inventory. */}
          <SavedSearchesPanel
            inventory={showroom}
            currentFilters={{
              category,
              q: search,
              maxPrice,
              beds,
              view,
              sort,
            }}
          />

          {filtered.length > 0 ? (
            <div className="grid gap-x-7 gap-y-14 md:grid-cols-2 xl:grid-cols-3">
              {filtered.map((property) => (
                <InventoryCard
                  key={property.id}
                  property={property}
                />
              ))}
            </div>
          ) : (
            <div className="border-y border-border py-20 text-center">
              <p className="font-serif text-3xl font-light text-foreground">
                No properties match this search.
              </p>

              <button
                type="button"
                onClick={() => {
                  setCategory('all')
                  setSearch('')
                  setMaxPrice('')
                  setBeds('')
                  setView('')
                }}
                className="mt-6 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground underline underline-offset-4 transition-colors hover:text-foreground"
              >
                Clear filters
              </button>
            </div>
          )}
        </div>
      </section>
    </>
  )
}