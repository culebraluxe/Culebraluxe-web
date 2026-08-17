import type { LucideIcon } from 'lucide-react'
import {
  Bath,
  BedDouble,
  CalendarDays,
  Car,
  Layers3,
  Maximize,
  Trees,
  Waves,
} from 'lucide-react'

import { formatArea, formatPrice, isLand } from '@/lib/property'
import type { PropertyDetail } from '@/lib/property-types'

export function bathroomsDisplay(p: PropertyDetail): string | null {
  const total =
    p.bathroomsTotal ??
    ((p.bathroomsFull ?? 0) + (p.bathroomsHalf ?? 0) || null)

  if (total == null) return null

  if (p.bathroomsFull != null || p.bathroomsHalf != null) {
    const parts: string[] = []

    if (p.bathroomsFull != null) {
      parts.push(`${p.bathroomsFull} Full`)
    }

    if (p.bathroomsHalf != null) {
      parts.push(`${p.bathroomsHalf} Half`)
    }

    return `${total} (${parts.join(', ')})`
  }

  return String(total)
}

export function DefinitionRow({
  label,
  value,
}: {
  label: string
  value?: string | number | null
}) {
  if (value == null || value === '') return null

  return (
    <div className="flex items-baseline justify-between gap-6 border-b border-border/60 py-3 last:border-0">
      <dt className="text-xs font-light uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </dt>

      <dd className="text-right text-sm font-light text-foreground">
        {value}
      </dd>
    </div>
  )
}

export function PropertyFactsCard({
  property,
  compact = false,
}: {
  property: PropertyDetail
  compact?: boolean
}) {
  const land = isLand(property.propertyType)

  if (compact) {
    const additionalFact =
      property.stories != null
        ? {
            icon: Layers3,
            label: 'Stories',
            value: property.stories,
          }
        : property.waterAccess
          ? {
              icon: Waves,
              label: 'Water Access',
              value: 'Yes',
            }
          : null

    const facts: {
      icon: LucideIcon
      label: string
      value: string | number | null | undefined
    }[] = [
      ...(!land
        ? [
            {
              icon: BedDouble,
              label: 'Beds',
              value: property.bedroomsTotal,
            },
            {
              icon: Bath,
              label: 'Baths',
              value: bathroomsDisplay(property),
            },
            {
              icon: Maximize,
              label: 'Interior',
              value:
                property.livingArea != null
                  ? `${property.livingArea.toLocaleString('en-US')} SF`
                  : null,
            },
          ]
        : []),
      ...(land
        ? [
            {
              icon: Trees,
              label: 'Lot',
              value: formatArea(
                property.lotSizeArea,
                property.lotSizeUnits,
              ),
            },
          ]
        : []),
      {
        icon: CalendarDays,
        label: 'Built',
        value: property.yearBuilt,
      },
      {
        icon: Car,
        label: 'Parking',
        value: property.parkingSpaces,
      },
      ...(additionalFact ? [additionalFact] : []),
    ]

    const visibleFacts = facts.filter(
      (fact) => fact.value != null && fact.value !== '',
    )

    return (
      <div className="px-5 py-3 lg:px-6">
        <p className="mb-1.5 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground/90">
          Key Facts
        </p>

        <dl className="grid grid-cols-2 gap-x-5 xl:grid-cols-3 xl:gap-x-4">
          {visibleFacts.map((fact) => (
            <div
              key={fact.label}
              className="flex min-h-12 items-center gap-2.5 border-b border-border/80 py-2"
            >
              <fact.icon
                className="h-3.5 w-3.5 flex-none self-center text-accent/90"
                aria-hidden
              />

              <div className="min-w-0">
                <dt className="text-[9px] font-light uppercase tracking-[0.14em] text-muted-foreground/90">
                  {fact.label}
                </dt>

                <dd className="truncate text-sm font-normal leading-tight text-foreground/90">
                  {fact.value}
                </dd>
              </div>
            </div>
          ))}
        </dl>
      </div>
    )
  }

  return (
    <div className="border border-border p-8">
      <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
        Key Facts
      </p>

      <dl>
        <DefinitionRow
          label="Price"
          value={formatPrice(property.listPrice)}
        />

        <DefinitionRow
          label="Property Type"
          value={property.propertyType}
        />

        <DefinitionRow
          label="Status"
          value={property.standardStatus}
        />

        {!land && (
          <>
            <DefinitionRow
              label="Bedrooms"
              value={property.bedroomsTotal}
            />

            <DefinitionRow
              label="Bathrooms"
              value={bathroomsDisplay(property)}
            />

            <DefinitionRow
              label="Living Area"
              value={
                property.livingArea != null
                  ? `${property.livingArea.toLocaleString('en-US')} SF`
                  : null
              }
            />
          </>
        )}

        <DefinitionRow
          label="Lot Size"
          value={formatArea(
            property.lotSizeArea,
            property.lotSizeUnits,
          )}
        />

        <DefinitionRow
          label="Year Built"
          value={property.yearBuilt}
        />

        <DefinitionRow
          label="Parking"
          value={property.parkingSpaces}
        />
      </dl>

      {(property.listingAgentName ||
        property.listingAgentPhone ||
        property.listingAgentEmail ||
        property.listingId) && (
        <>
          <p className="mb-4 mt-8 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
            Listing Information
          </p>

          <dl>
            <DefinitionRow
              label="Listing Agent"
              value={property.listingAgentName}
            />

            <DefinitionRow
              label="Office"
              value={property.listingOffice}
            />

            <DefinitionRow
              label="Phone"
              value={property.listingAgentPhone}
            />

            <DefinitionRow
              label="Email"
              value={property.listingAgentEmail}
            />

            <DefinitionRow
              label="MLS / Listing ID"
              value={property.listingId}
            />
          </dl>
        </>
      )}
    </div>
  )
}
