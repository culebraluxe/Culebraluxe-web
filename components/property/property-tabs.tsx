'use client'

import { useState } from 'react'
import { Check } from 'lucide-react'
import MuxPlayer from '@mux/mux-player-react'

import {
  PropertyGallery,
  type GalleryImage,
} from '@/components/property/property-gallery'
import { formatArea, formatPrice, isLand } from '@/lib/property'
import { cn } from '@/lib/utils'

export type PropertyDetail = {
  _id: string
  title?: string | null
  listingId?: string | null
  standardStatus?: string | null
  propertyType?: string | null
  listPrice?: number | null
  city?: string | null
  stateOrProvince?: string | null
  neighborhood?: string | null
  latitude?: number | null
  longitude?: number | null
  bedroomsTotal?: number | null
  bathroomsFull?: number | null
  bathroomsHalf?: number | null
  bathroomsTotal?: number | null
  livingArea?: number | null
  lotSizeArea?: number | null
  lotSizeUnits?: string | null
  yearBuilt?: number | null
  stories?: number | null
  parkingSpaces?: number | null
  viewType?: string[] | null
  waterAccess?: boolean | null
  beachAccess?: boolean | null
  amenities?: string[] | null
  shortDescription?: string | null
  editorialDescription?: string | null
  architecture?: string | null
  lifestyleTags?: string[] | null
  listingAgentName?: string | null
  listingAgentEmail?: string | null
  listingAgentPhone?: string | null
  listingOffice?: string | null
}

export type PropertyVideo = {
  id: string
  playbackId: string
  role: 'video' | 'short'
  title: string
  caption?: string | null
  aspectRatio?: string | null
  durationSeconds?: number | null
}

type Tab =
  | 'Overview'
  | 'Details'
  | 'Features'
  | 'Location'
  | 'Gallery'
  | 'Video'

const BASE_TABS: Tab[] = [
  'Overview',
  'Details',
  'Features',
  'Location',
  'Gallery',
]

function bathroomsDisplay(p: PropertyDetail): string | null {
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

function DefinitionRow({
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
          className="text-pretty text-[15px] font-light leading-relaxed text-foreground/80"
        >
          {paragraph}
        </p>
      ))}
    </div>
  )
}

function playerAspectRatio(video: PropertyVideo) {
  if (!video.aspectRatio) {
    return video.role === 'short' ? '9 / 16' : '16 / 9'
  }

  return video.aspectRatio.replace(':', ' / ')
}

function PropertyVideos({ videos }: { videos: PropertyVideo[] }) {
  const films = videos.filter((video) => video.role === 'video')
  const shorts = videos.filter((video) => video.role === 'short')

  return (
    <div className="flex flex-col gap-16">
      {films.length > 0 && (
        <div>
          <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
            Property Film
          </p>

          <div className="flex flex-col gap-10">
            {films.map((video) => (
              <div key={video.id}>
                <MuxPlayer
                  playbackId={video.playbackId}
                  metadata={{
                    video_title: video.title,
                  }}
                  style={{
                    width: '100%',
                    aspectRatio: playerAspectRatio(video),
                  }}
                />

                {video.caption && (
                  <p className="mt-4 text-sm font-light leading-relaxed text-muted-foreground">
                    {video.caption}
                  </p>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {shorts.length > 0 && (
        <div>
          <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
            Short Films
          </p>

          <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
            {shorts.map((video) => (
              <div key={video.id}>
                <MuxPlayer
                  playbackId={video.playbackId}
                  metadata={{
                    video_title: video.title,
                  }}
                  style={{
                    width: '100%',
                    aspectRatio: playerAspectRatio(video),
                  }}
                />

                {video.caption && (
                  <p className="mt-3 text-sm font-light leading-relaxed text-muted-foreground">
                    {video.caption}
                  </p>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

export function PropertyTabs({
  property,
  galleryImages,
  videos = [],
}: {
  property: PropertyDetail
  galleryImages: GalleryImage[]
  videos?: PropertyVideo[]
}) {
  const [active, setActive] = useState<Tab>('Overview')
  const land = isLand(property.propertyType)

  const tabs: Tab[] =
    videos.length > 0 ? [...BASE_TABS, 'Video'] : BASE_TABS

  const highlights = (property.amenities ?? []).slice(0, 8)

  const hasEditorial =
    typeof property.editorialDescription === 'string' &&
    property.editorialDescription.trim().length > 0

  const locationLine = [
    property.neighborhood,
    property.city,
    property.stateOrProvince,
  ]
    .filter(Boolean)
    .join(', ')

  return (
    <section className="w-full">
      <nav
        className="flex flex-wrap gap-x-10 gap-y-3 border-b border-border"
        aria-label="Property information"
      >
        {tabs.map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActive(tab)}
            aria-current={active === tab}
            className={cn(
              'relative -mb-px pb-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-300',
              active === tab
                ? 'text-foreground'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            {tab}

            <span
              className={cn(
                'absolute -bottom-px left-0 h-px w-full transition-colors duration-300',
                active === tab ? 'bg-foreground' : 'bg-transparent',
              )}
            />
          </button>
        ))}
      </nav>

      <div className="pt-12">
        {active === 'Overview' && (
          <div className="grid gap-12 lg:grid-cols-12 lg:gap-16">
            <div className="lg:col-span-7">
              <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-accent">
                The Property
              </p>

              {hasEditorial ? (
                <EditorialText value={property.editorialDescription!} />
              ) : property.shortDescription ? (
                <p className="text-pretty text-lg font-light leading-relaxed text-foreground/80">
                  {property.shortDescription}
                </p>
              ) : (
                <p className="text-sm font-light text-muted-foreground">
                  A detailed description of this residence is being prepared.
                </p>
              )}

              {highlights.length > 0 && (
                <div className="mt-12">
                  <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                    Highlights
                  </p>

                  <ul className="grid gap-x-8 gap-y-4 sm:grid-cols-2">
                    {highlights.map((item) => (
                      <li key={item} className="flex items-start gap-3">
                        <Check className="mt-0.5 h-4 w-4 flex-none text-accent" />

                        <span className="text-sm font-light leading-snug text-foreground/85">
                          {item}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>

            <aside className="lg:col-span-5">
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
            </aside>
          </div>
        )}

        {active === 'Details' && (
          <div className="max-w-3xl">
            <dl className="grid gap-x-16 sm:grid-cols-2">
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

                  <DefinitionRow
                    label="Stories"
                    value={property.stories}
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
                label="Parking Spaces"
                value={property.parkingSpaces}
              />

              <DefinitionRow
                label="Neighborhood"
                value={property.neighborhood}
              />

              <DefinitionRow
                label="Water Access"
                value={property.waterAccess ? 'Yes' : null}
              />

              <DefinitionRow
                label="Beach Access"
                value={property.beachAccess ? 'Yes' : null}
              />
            </dl>
          </div>
        )}

        {active === 'Features' && (
          <div className="flex flex-col gap-12">
            {(property.amenities ?? []).length > 0 && (
              <div>
                <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                  Amenities
                </p>

                <ul className="grid gap-x-8 gap-y-4 sm:grid-cols-2 lg:grid-cols-3">
                  {property.amenities!.map((item) => (
                    <li key={item} className="flex items-start gap-3">
                      <Check className="mt-0.5 h-4 w-4 flex-none text-accent" />

                      <span className="text-sm font-light leading-snug text-foreground/85">
                        {item}
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {(property.viewType ?? []).length > 0 && (
              <div>
                <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                  Views & Setting
                </p>

                <div className="flex flex-wrap gap-3">
                  {property.viewType!.map((view) => (
                    <span
                      key={view}
                      className="border border-border px-4 py-2 text-xs font-light uppercase tracking-[0.16em] text-foreground/80"
                    >
                      {view}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {(property.lifestyleTags ?? []).length > 0 && (
              <div>
                <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                  Lifestyle
                </p>

                <div className="flex flex-wrap gap-3">
                  {property.lifestyleTags!.map((tag) => (
                    <span
                      key={tag}
                      className="border border-border px-4 py-2 text-xs font-light uppercase tracking-[0.16em] text-foreground/80"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {(property.amenities ?? []).length === 0 &&
              (property.viewType ?? []).length === 0 &&
              (property.lifestyleTags ?? []).length === 0 && (
                <p className="text-sm font-light text-muted-foreground">
                  Feature details will be added shortly.
                </p>
              )}
          </div>
        )}

        {active === 'Location' && (
          <div className="flex flex-col gap-8">
            {locationLine && (
              <div>
                <p className="mb-2 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                  Location
                </p>

                <p className="font-serif text-2xl font-light text-foreground">
                  {locationLine}
                </p>
              </div>
            )}

            {property.latitude != null &&
            property.longitude != null ? (
              <div className="aspect-[16/9] w-full overflow-hidden border border-border">
                <iframe
                  title={`Map of ${property.title ?? 'property'}`}
                  className="h-full w-full"
                  loading="lazy"
                  referrerPolicy="no-referrer-when-downgrade"
                  src={`https://www.openstreetmap.org/export/embed.html?bbox=${
                    property.longitude - 0.012
                  }%2C${property.latitude - 0.008}%2C${
                    property.longitude + 0.012
                  }%2C${property.latitude + 0.008}&layer=mapnik&marker=${
                    property.latitude
                  }%2C${property.longitude}`}
                />
              </div>
            ) : (
              <p className="text-sm font-light text-muted-foreground">
                Precise location is shared privately with qualified buyers.
              </p>
            )}
          </div>
        )}

        {active === 'Gallery' && (
          <PropertyGallery images={galleryImages} />
        )}

        {active === 'Video' && videos.length > 0 && (
          <PropertyVideos videos={videos} />
        )}
      </div>
    </section>
  )
}