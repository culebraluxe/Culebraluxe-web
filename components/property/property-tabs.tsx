'use client'

import { useState } from 'react'
import { Compass, Eye, Landmark, Sparkles } from 'lucide-react'

import { bathroomsDisplay } from '@/components/property/property-facts-card'
import { PropertyLocation } from '@/components/property/property-location'
import { PropertyVideos } from '@/components/property/property-videos'
import { formatArea, isLand } from '@/lib/property'
import type {
  PropertyDetail,
  PropertyVideo,
} from '@/lib/property-types'
import { cn } from '@/lib/utils'

type Tab =
  | 'Overview'
  | 'Details'
  | 'Map'
  | 'Video'

const PRIMARY_TABS: Tab[] = [
  'Overview',
  'Details',
]

const GENERATED_AMENITIES = new Set([
  'Pool',
  'Whole-Home Generator',
  'Solar Power',
  'Furnished',
  'Gated',
  'Water Access',
  'Beach Access',
])

const GENERATED_LIFESTYLE_TAGS = new Set([
  'Ocean View',
  'Sunset View',
  'Sunrise View',
  'Beach Access',
  'Water Access',
  'Pool',
  'Private Estate',
])

function EditorialText({ value }: { value: string }) {
  const paragraphs = value
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.trim())
    .filter(Boolean)

  return (
    <div className="flex flex-col gap-5">
      {paragraphs.map((paragraph, index) => (
        <p
          key={`${index}-${paragraph.slice(0, 32)}`}
          className="text-pretty text-[15px] font-normal leading-relaxed text-brand-navy/90"
        >
          {paragraph}
        </p>
      ))}
    </div>
  )
}

export function PropertyTabs({
  property,
  videos = [],
}: {
  property: PropertyDetail
  videos?: PropertyVideo[]
}) {
  const [active, setActive] = useState<Tab>('Overview')
  const land = isLand(property.propertyType)

  const tabs: Tab[] = [
    ...PRIMARY_TABS,
    ...(videos.length > 0 ? (['Video'] as const) : []),
    'Map',
  ]

  const amenities = property.amenities ?? []
  const views = property.viewType ?? []
  const lifestyleTags = property.lifestyleTags ?? []
  const compactAmenities = amenities.filter((item) =>
    GENERATED_AMENITIES.has(item),
  )
  const amenityNotes = amenities.filter(
    (item) => !GENERATED_AMENITIES.has(item),
  )
  const visibleAmenityLabels = new Set(
    compactAmenities.map((item) => item.toLowerCase()),
  )
  const visibleViewLabels = new Set(
    views.flatMap((view) => [
      view.toLowerCase(),
      `${view.toLowerCase()} view`,
    ]),
  )
  const compactLifestyleTags = lifestyleTags.filter((item) => {
    if (!GENERATED_LIFESTYLE_TAGS.has(item)) return false

    const normalized = item.toLowerCase()

    return (
      !visibleAmenityLabels.has(normalized) &&
      !visibleViewLabels.has(normalized)
    )
  })
  const lifestyleNotes = lifestyleTags.filter(
    (item) => !GENERATED_LIFESTYLE_TAGS.has(item),
  )
  const hasLeftHighlights =
    compactAmenities.length > 0 ||
    amenityNotes.length > 0 ||
    Boolean(property.architecture)
  const hasViews = views.length > 0
  const hasLifestyle =
    compactLifestyleTags.length > 0 || lifestyleNotes.length > 0

  const overviewFacts = [
    {
      label: 'Lot Size',
      value: formatArea(
        property.lotSizeArea,
        property.lotSizeUnits,
      ),
    },
    {
      label: 'Neighborhood',
      value: property.neighborhood,
    },
  ].filter((fact) => fact.value != null && fact.value !== '')

  const listingFacts = [
    {
      label: 'Listing Agent',
      value: property.listingAgentName,
    },
    {
      label: 'Office',
      value: property.listingOffice,
    },
    {
      label: 'Phone',
      value: property.listingAgentPhone,
    },
    {
      label: 'Email',
      value: property.listingAgentEmail,
    },
    {
      label: 'MLS / Listing ID',
      value: property.listingId,
    },
  ].filter((fact) => fact.value != null && fact.value !== '')

  const hasOverviewSidebar =
    overviewFacts.length > 0 ||
    listingFacts.length > 0 ||
    hasViews ||
    hasLifestyle

  const detailFacts = [
    {
      label: 'Property Type',
      value: property.propertyType,
    },
    {
      label: 'Status',
      value: property.standardStatus,
    },
    ...(!land
      ? [
          {
            label: 'Bedrooms',
            value: property.bedroomsTotal,
          },
          {
            label: 'Bathrooms',
            value: bathroomsDisplay(property),
          },
          {
            label: 'Living Area',
            value:
              property.livingArea != null
                ? `${property.livingArea.toLocaleString('en-US')} SF`
                : null,
          },
          {
            label: 'Stories',
            value: property.stories,
          },
        ]
      : []),
    {
      label: 'Lot Size',
      value: formatArea(
        property.lotSizeArea,
        property.lotSizeUnits,
      ),
    },
    {
      label: 'Year Built',
      value: property.yearBuilt,
    },
    {
      label: 'Parking Spaces',
      value: property.parkingSpaces,
    },
    {
      label: 'Neighborhood',
      value: property.neighborhood,
    },
    {
      label: 'Water Access',
      value: property.waterAccess ? 'Yes' : null,
    },
    {
      label: 'Beach Access',
      value: property.beachAccess ? 'Yes' : null,
    },
  ].filter((fact) => fact.value != null && fact.value !== '')

  const hasEditorial =
    typeof property.editorialDescription === 'string' &&
    property.editorialDescription.trim().length > 0

  return (
    <section className="w-full border-x border-b border-brand-navy/35 bg-card">
      <nav
        className="flex h-[60px] flex-nowrap overflow-x-auto border-b border-brand-navy/35 bg-card sm:h-16"
        aria-label="Property information"
      >
        {tabs.map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActive(tab)}
            aria-current={active === tab ? 'page' : undefined}
            className={cn(
              'relative flex min-h-12 min-w-[116px] flex-none self-stretch items-center justify-center border-r border-[#c6a15b] px-6 font-serif text-sm tracking-[0.04em] transition-colors duration-300 sm:min-w-[132px] sm:px-8 sm:text-[15px]',
              active === tab
                ? 'bg-brand-navy font-semibold text-[#c6a15b]'
                : 'bg-brand-navy font-medium text-[#f8f5ec]/85 hover:text-[#f8f5ec]',
            )}
          >
            {tab}

            <span
              className={cn(
                'absolute inset-x-0 bottom-0 h-0.5 transition-colors duration-300',
                active === tab ? 'bg-[#c6a15b]' : 'bg-transparent',
              )}
            />
          </button>
        ))}
      </nav>

      <div className="bg-brand-navy/[0.035] shadow-[inset_0_0_0_1px_rgba(3,15,35,0.16)] p-5 sm:p-6 lg:p-7">
        {active === 'Overview' && (
          <div
            className={cn(
              'grid items-start gap-8',
              hasOverviewSidebar
                ? 'lg:grid-cols-[minmax(0,1.65fr)_minmax(280px,1fr)] lg:gap-12'
                : 'max-w-4xl',
            )}
          >
            <div className="min-w-0">
              <p className="mb-4 text-xs font-medium uppercase tracking-[0.34em] text-[#c6a15b]">
                The Property
              </p>

              <div className="max-w-[68ch]">
                {hasEditorial ? (
                  <EditorialText value={property.editorialDescription!} />
                ) : property.shortDescription ? (
                  <p className="text-pretty text-base font-normal leading-7 text-brand-navy/90">
                    {property.shortDescription}
                  </p>
                ) : (
                  <p className="text-sm font-normal leading-relaxed text-brand-navy/80">
                    A detailed description of this residence is being prepared.
                  </p>
                )}
              </div>

              {hasLeftHighlights && (
                <div className="mt-7 border-t border-brand-navy/20 pt-6">
                  <p className="mb-5 text-xs font-medium uppercase tracking-[0.24em] text-[#c6a15b]">
                    Property Highlights
                  </p>

                  <div className="grid gap-x-10 gap-y-6 sm:grid-cols-2">
                    {(compactAmenities.length > 0 || amenityNotes.length > 0) && (
                      <section className="min-w-0">
                        <div className="mb-3 flex items-center gap-2.5">
                          <Sparkles
                            className="h-4 w-4 flex-none text-[#c6a15b]"
                            aria-hidden
                          />

                          <h3 className="font-serif text-base font-semibold text-brand-navy">
                            Amenities
                          </h3>
                        </div>

                        {compactAmenities.length > 0 && (
                          <ul className="space-y-2 pl-6">
                            {compactAmenities.map((item) => (
                              <li
                                key={item}
                                className="relative text-sm font-normal leading-snug text-brand-navy/90 before:absolute before:-left-4 before:top-[0.55em] before:h-px before:w-1.5 before:bg-[#c6a15b]/70"
                              >
                                {item}
                              </li>
                            ))}
                          </ul>
                        )}

                        {amenityNotes.map((note) => (
                          <p
                            key={note}
                            className={cn(
                              'border-l border-[#c6a15b]/50 pl-4 text-sm font-normal leading-relaxed text-brand-navy/82',
                              compactAmenities.length > 0 && 'mt-4',
                            )}
                          >
                            {note}
                          </p>
                        ))}
                      </section>
                    )}

                    {property.architecture && (
                      <section className="min-w-0">
                        <div className="mb-3 flex items-center gap-2.5">
                          <Landmark
                            className="h-4 w-4 flex-none text-[#c6a15b]"
                            aria-hidden
                          />

                          <h3 className="font-serif text-base font-semibold text-brand-navy">
                            Architecture
                          </h3>
                        </div>

                        <p className="border-l border-[#c6a15b]/50 pl-4 text-sm font-normal leading-relaxed text-brand-navy/82">
                          {property.architecture}
                        </p>
                      </section>
                    )}
                  </div>
                </div>
              )}
            </div>

            {hasOverviewSidebar && (
              <aside className="min-w-0 self-start space-y-6">
                {overviewFacts.length === 1 &&
                  overviewFacts[0].label === 'Neighborhood' && (
                    <dl
                      className="border-t border-brand-navy/40 bg-brand-navy/[0.05] px-5 py-4 ring-1 ring-inset ring-brand-navy/20 sm:px-6"
                      aria-label="Location context"
                    >
                      <div className="flex items-baseline justify-between gap-5">
                        <dt className="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">
                          Neighborhood
                        </dt>

                        <dd className="text-right text-[15px] font-medium text-brand-navy">
                          {overviewFacts[0].value}
                        </dd>
                      </div>
                    </dl>
                  )}

                {(overviewFacts.length > 1 ||
                  (overviewFacts.length === 1 &&
                    overviewFacts[0].label !== 'Neighborhood')) && (
                  <section className="border-t border-brand-navy/40 bg-brand-navy/[0.05] px-5 py-5 ring-1 ring-inset ring-brand-navy/20 sm:px-6">
                    <p className="mb-4 text-xs font-medium uppercase tracking-[0.24em] text-[#c6a15b]">
                      Key Facts
                    </p>

                    <dl className="grid grid-cols-2 gap-x-6 gap-y-4">
                      {overviewFacts.map((fact) => (
                        <div key={fact.label} className="min-w-0">
                          <dt className="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">
                            {fact.label}
                          </dt>

                          <dd className="mt-1 break-words text-[15px] font-medium leading-snug text-brand-navy">
                            {fact.value}
                          </dd>
                        </div>
                      ))}
                    </dl>
                  </section>
                )}

                {listingFacts.length > 0 && (
                  <section className="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20 sm:px-6">
                    <p className="mb-3 text-xs font-medium uppercase tracking-[0.2em] text-[#c6a15b]">
                      Listing Information
                    </p>

                    <dl className="space-y-3">
                      {listingFacts.map((fact) => (
                        <div
                          key={fact.label}
                          className="grid grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] gap-4"
                        >
                          <dt className="text-[10px] font-semibold uppercase tracking-[0.14em] text-brand-navy/70">
                            {fact.label}
                          </dt>

                          <dd className="break-words text-right text-sm font-medium leading-snug text-brand-navy">
                            {fact.value}
                          </dd>
                        </div>
                      ))}
                    </dl>
                  </section>
                )}

                {hasViews && (
                  <section className="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20 sm:px-6">
                    <div className="mb-3 flex items-center gap-2.5">
                      <Eye
                        className="h-4 w-4 flex-none text-[#c6a15b]"
                        aria-hidden
                      />

                      <h3 className="font-serif text-base font-semibold text-brand-navy">
                        Views
                      </h3>
                    </div>

                    <ul className="grid grid-cols-2 gap-x-5 gap-y-2 pl-6">
                      {views.map((view) => (
                        <li
                          key={view}
                          className="relative text-sm font-normal leading-snug text-brand-navy/90 before:absolute before:-left-4 before:top-[0.55em] before:h-px before:w-1.5 before:bg-[#c6a15b]/70"
                        >
                          {view}
                        </li>
                      ))}
                    </ul>
                  </section>
                )}

                {hasLifestyle && (
                  <section className="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20 sm:px-6">
                    <div className="mb-3 flex items-center gap-2.5">
                      <Compass
                        className="h-4 w-4 flex-none text-[#c6a15b]"
                        aria-hidden
                      />

                      <h3 className="font-serif text-base font-semibold text-brand-navy">
                        Lifestyle
                      </h3>
                    </div>

                    {compactLifestyleTags.length > 0 && (
                      <ul className="grid grid-cols-2 gap-x-5 gap-y-2 pl-6">
                        {compactLifestyleTags.map((tag) => (
                          <li
                            key={tag}
                            className="relative text-sm font-normal leading-snug text-brand-navy/90 before:absolute before:-left-4 before:top-[0.55em] before:h-px before:w-1.5 before:bg-[#c6a15b]/70"
                          >
                            {tag}
                          </li>
                        ))}
                      </ul>
                    )}

                    {lifestyleNotes.map((note) => (
                      <p
                        key={note}
                        className={cn(
                          'border-l border-[#c6a15b]/50 pl-4 text-sm font-normal leading-relaxed text-brand-navy/82',
                          compactLifestyleTags.length > 0 && 'mt-4',
                        )}
                      >
                        {note}
                      </p>
                    ))}
                  </section>
                )}
              </aside>
            )}
          </div>
        )}

        {active === 'Details' && (
          <div>
            <dl className="grid grid-cols-1 gap-x-8 sm:grid-cols-2 lg:grid-cols-3 lg:gap-x-10">
              {detailFacts.map((fact) => (
                <div
                  key={fact.label}
                  className="min-w-0 border-t border-brand-navy/40 bg-card/60 px-3 py-4 ring-1 ring-inset ring-brand-navy/25"
                >
                  <dt className="text-[10px] font-semibold uppercase tracking-[0.17em] text-brand-navy/70">
                    {fact.label}
                  </dt>

                  <dd className="mt-1.5 break-words text-[15px] font-medium leading-snug text-brand-navy">
                    {fact.value}
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        )}

        {active === 'Map' && (
          <PropertyLocation property={property} />
        )}

        {active === 'Video' && videos.length > 0 && (
          <PropertyVideos videos={videos} />
        )}
      </div>
    </section>
  )
}
