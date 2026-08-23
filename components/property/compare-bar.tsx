'use client'

import Image from 'next/image'
import Link from 'next/link'
import { useEffect, useState } from 'react'
import { X } from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import {
  COMPARE_CHANGED_EVENT,
  pruneCompare,
  removeCompare,
} from '@/lib/compare'
import { formatArea, formatPrice, isLand, propertyLocation } from '@/lib/property'

type CompareBarProps = {
  properties: PropertySummary[]
}

export function CompareBar({ properties }: CompareBarProps) {
  const [mounted, setMounted] = useState(false)
  const [selected, setSelected] = useState<PropertySummary[]>([])

  useEffect(() => {
    setMounted(true)

    const refresh = () => {
      const pruned = pruneCompare(properties.map((property) => property.slug))
      const matched = pruned
        .map((entry) => properties.find((property) => property.slug === entry.slug))
        .filter((property): property is PropertySummary => Boolean(property))
      setSelected(matched)
    }

    refresh()
    window.addEventListener(COMPARE_CHANGED_EVENT, refresh)
    return () => window.removeEventListener(COMPARE_CHANGED_EVENT, refresh)
  }, [properties])

  if (!mounted || selected.length < 2) {
    return null
  }

  const rows: Array<{ label: string; value: (property: PropertySummary) => string }> = [
    { label: 'Price', value: (property) => formatPrice(property.listPrice) },
    { label: 'Location', value: (property) => propertyLocation(property) ?? '—' },
    { label: 'Type', value: (property) => property.propertyType ?? '—' },
    {
      label: 'Beds',
      value: (property) =>
        isLand(property.propertyType)
          ? '—'
          : property.bedrooms != null
            ? String(property.bedrooms)
            : '—',
    },
    {
      label: 'Baths',
      value: (property) =>
        isLand(property.propertyType)
          ? '—'
          : property.bathrooms != null
            ? String(property.bathrooms)
            : '—',
    },
    {
      label: 'Interior',
      value: (property) =>
        isLand(property.propertyType)
          ? '—'
          : property.squareFeet != null
            ? `${property.squareFeet.toLocaleString('en-US')} SF`
            : '—',
    },
    {
      label: 'Lot',
      value: (property) => formatArea(property.lotSize, property.lotSizeUnits) ?? '—',
    },
    { label: 'Views', value: (property) => property.views.join(', ') || '—' },
    { label: 'Status', value: (property) => property.status ?? '—' },
    { label: 'Water access', value: (property) => (property.waterAccess ? 'Yes' : 'No') },
    { label: 'Beach access', value: (property) => (property.beachAccess ? 'Yes' : 'No') },
  ]

  return (
    <section className="mt-20 border-t border-border pt-12">
      <div className="mb-8 flex flex-wrap items-center justify-between gap-4">
        <div>
          <p className="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">
            Compare
          </p>
          <h2 className="font-serif text-3xl font-light leading-tight text-foreground">
            Side by side
          </h2>
        </div>
        <p className="text-xs font-light text-muted-foreground">
          {selected.length} of 3 selected
        </p>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full min-w-[640px] border-collapse">
          <thead>
            <tr>
              <th className="w-40 border-b border-border pb-4 pr-6 text-left align-bottom">
                <span className="text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                  Property
                </span>
              </th>
              {selected.map((property) => (
                <th
                  key={property.id}
                  className="border-b border-border pb-4 pr-6 text-left align-bottom"
                >
                  <div className="relative mb-4 aspect-[4/3] w-44 overflow-hidden bg-muted">
                    {property.heroUrl ? (
                      <Image
                        src={property.heroUrl}
                        alt={property.heroAlt}
                        fill
                        unoptimized
                        sizes="176px"
                        className="object-cover"
                      />
                    ) : (
                      <div className="h-full w-full bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]" />
                    )}

                    <button
                      type="button"
                      onClick={() => removeCompare(property.id)}
                      aria-label={`Remove ${property.name} from compare`}
                      className="absolute right-2 top-2 flex h-10 w-10 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>

                  <Link
                    href={`/properties/${property.slug}`}
                    className="font-serif text-xl font-light leading-tight text-foreground transition-colors hover:text-accent"
                  >
                    {property.name}
                  </Link>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.label}>
                <td className="border-b border-border py-4 pr-6 text-[10px] font-light uppercase tracking-[0.16em] text-muted-foreground">
                  {row.label}
                </td>
                {selected.map((property) => (
                  <td
                    key={property.id}
                    className="border-b border-border py-4 pr-6 text-sm font-light text-foreground"
                  >
                    {row.value(property)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}
