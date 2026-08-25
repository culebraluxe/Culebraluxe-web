'use client'

import Image from 'next/image'
import Link from 'next/link'
import {
  useEffect,
  useRef,
  useState,
} from 'react'
import {
  ArrowLeft,
  ArrowRight,
} from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import {
  formatPrice,
  isLand,
  propertyFactParts,
  propertyLocation,
} from '@/lib/property'

type FeaturedPropertyCarouselProps = {
  properties: PropertySummary[]
}

function PropertyCard({
  property,
  index,
}: {
  property: PropertySummary
  index: number
}) {
  const href = `/properties/${property.slug}`
  const loc = propertyLocation(property)

  return (
    <article
      data-carousel-card
      className="
        group
        relative
        h-full
        w-full
        shrink-0
        snap-start
        overflow-hidden
        bg-foreground
        sm:w-[calc(50%-6px)]
        lg:w-[calc(33.333333%-8px)]
        xl:w-[calc(25%-9px)]
      "
    >
      {property.heroUrl ? (
        <Image
          src={property.heroUrl}
          alt={property.heroAlt}
          fill
          unoptimized
          sizes="
            (min-width: 1280px) 25vw,
            (min-width: 1024px) 33vw,
            (min-width: 640px) 50vw,
            100vw
          "
          className="
            object-cover
            transition-transform
            duration-[1400ms]
            ease-[cubic-bezier(0.22,1,0.36,1)]
            group-hover:scale-[1.035]
          "
        />
      ) : (
        <div className="absolute inset-0 bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]" />
      )}

      <div
        aria-hidden
        className="
          absolute
          inset-0
          bg-gradient-to-t
          from-black/85
          via-black/15
          to-black/5
        "
      />

      <Link
        href={href}
        aria-label={`View ${property.name}`}
        className="absolute inset-0 z-10"
      />

      <div className="pointer-events-none absolute left-5 top-5 z-20">
        <span className="text-[10px] font-light tracking-[0.2em] text-white/65">
          {String(index + 1).padStart(2, '0')}
        </span>
      </div>

      <div className="pointer-events-none absolute right-5 top-5 z-20 flex flex-col items-end gap-2">
        {property.featured && (
          <span className="border border-white/35 bg-black/15 px-2.5 py-1 text-[9px] font-light uppercase tracking-[0.18em] text-white backdrop-blur-sm">
            Featured
          </span>
        )}

        {isLand(property.propertyType) && (
          <span className="border border-white/35 bg-black/15 px-2.5 py-1 text-[9px] font-light uppercase tracking-[0.18em] text-white backdrop-blur-sm">
            Land
          </span>
        )}

        {property.status ===
          'under_contract' && (
          <span className="border border-white/35 bg-black/15 px-2.5 py-1 text-[9px] font-light uppercase tracking-[0.18em] text-white backdrop-blur-sm">
            Under Contract
          </span>
        )}
      </div>

      <div className="pointer-events-none absolute inset-x-0 bottom-0 z-20 p-5 text-white md:p-6">
        {loc ? (
          <p className="mb-2 text-[9px] font-light uppercase tracking-[0.2em] text-white/65">
            {loc}
          </p>
        ) : null}

        <h3 className="font-serif text-2xl font-light leading-[1.05] md:text-[28px]">
          {property.name}
        </h3>

        <p className="mt-3 text-[10px] font-light uppercase tracking-[0.12em] text-white/65">
          {propertyFactParts(property).join('  ·  ')}
        </p>

        <div className="mt-5 flex items-center justify-between gap-4 border-t border-white/20 pt-4">
          <p className="text-sm font-light text-white">
            {formatPrice(
              property.listPrice,
            )}
          </p>

          <ArrowRight className="h-4 w-4 text-white/70 transition-transform duration-500 group-hover:translate-x-1" />
        </div>
      </div>
    </article>
  )
}

export function FeaturedPropertyCarousel({
  properties,
}: FeaturedPropertyCarouselProps) {
  const railRef =
    useRef<HTMLDivElement | null>(null)
  const [reducedMotion, setReducedMotion] =
    useState(false)

  // Respect the visitor's motion preference for the scroll animation so the
  // carousel never animates against an explicit reduced-motion setting.
  useEffect(() => {
    const media = window.matchMedia(
      '(prefers-reduced-motion: reduce)',
    )
    setReducedMotion(media.matches)
    const onChange = () => setReducedMotion(media.matches)
    media.addEventListener('change', onChange)
    return () => media.removeEventListener('change', onChange)
  }, [])

  function move(direction: -1 | 1) {
    const rail = railRef.current

    if (!rail) {
      return
    }

    const firstCard =
      rail.querySelector<HTMLElement>(
        '[data-carousel-card]',
      )

    if (!firstCard) {
      return
    }

    const cardWidth =
      firstCard.getBoundingClientRect().width

    rail.scrollBy({
      left: direction * (cardWidth + 12),
      behavior: reducedMotion ? 'auto' : 'smooth',
    })
  }

  if (properties.length === 0) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-muted">
        <p className="text-sm font-light text-muted-foreground">
          No featured properties available.
        </p>
      </div>
    )
  }

  return (
    <div className="relative h-full w-full min-w-0 overflow-hidden">
      <div
        ref={railRef}
        className="
          flex
          h-full
          w-full
          min-w-0
          snap-x
          snap-mandatory
          gap-3
          overflow-x-auto
          overflow-y-hidden
          scroll-smooth
          [scrollbar-width:none]
          [&::-webkit-scrollbar]:hidden
        "
      >
        {properties.map(
          (property, index) => (
            <PropertyCard
              key={property.id}
              property={property}
              index={index}
            />
          ),
        )}
      </div>

      {properties.length > 1 && (
        <>
          <button
            type="button"
            onClick={() => move(-1)}
            aria-label="Previous properties"
            className="
              absolute
              left-3
              top-1/2
              z-30
              flex
              h-11
              w-11
              -translate-y-1/2
              items-center
              justify-center
              border
              border-white/40
              bg-black/25
              text-white
              backdrop-blur-md
              transition-colors
              hover:bg-white
              hover:text-black
              md:left-4
            "
          >
            <ArrowLeft className="h-4 w-4" />
          </button>

          <button
            type="button"
            onClick={() => move(1)}
            aria-label="Next properties"
            className="
              absolute
              right-3
              top-1/2
              z-30
              flex
              h-11
              w-11
              -translate-y-1/2
              items-center
              justify-center
              border
              border-white/40
              bg-black/25
              text-white
              backdrop-blur-md
              transition-colors
              hover:bg-white
              hover:text-black
              md:right-4
            "
          >
            <ArrowRight className="h-4 w-4" />
          </button>
        </>
      )}
    </div>
  )
}