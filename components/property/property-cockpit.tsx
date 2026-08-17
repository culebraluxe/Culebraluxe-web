import { MapPin } from 'lucide-react'

import { PropertyActions } from '@/components/property/property-actions'
import { PropertyFactsCard } from '@/components/property/property-facts-card'
import { PropertyMediaPanel } from '@/components/property/property-media-panel'
import { formatPrice } from '@/lib/property'
import type { GalleryImage, PropertyDetail } from '@/lib/property-types'

export function PropertyCockpit({
  property,
  heroUrl,
  galleryImages,
}: {
  property: PropertyDetail
  heroUrl: string | null
  galleryImages: GalleryImage[]
}) {
  const locationLine = [
    property.neighborhood,
    property.city,
    property.stateOrProvince,
  ]
    .filter(Boolean)
    .join(', ')

  return (
    <section className="overflow-hidden border border-border/90 bg-background lg:grid lg:min-h-[432px] lg:grid-cols-[minmax(0,1.5fr)_minmax(380px,1fr)]">
      <div className="relative h-[290px] sm:h-[370px] lg:h-[432px]">
        <PropertyMediaPanel
          heroUrl={heroUrl}
          galleryImages={galleryImages}
          propertyTitle={property.title}
        />
      </div>

      <div className="flex flex-col border-l-0 border-border/90 bg-background lg:border-l">
        <div className="border-b border-border/90 px-5 py-3.5 lg:px-6">
          <div className="mb-2 flex items-center gap-2">
            {locationLine && (
              <p className="flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.18em] text-muted-foreground">
                <MapPin className="h-3.5 w-3.5 text-accent" aria-hidden />
                {locationLine}
              </p>
            )}
          </div>

          <h1 className="text-balance font-serif text-2xl font-light leading-tight text-foreground xl:text-3xl">
            {property.title}
          </h1>

          <p className="mt-1.5 text-lg font-light text-foreground">
            {formatPrice(property.listPrice)}
          </p>

          {(property.propertyType || property.standardStatus) && (
            <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] font-light uppercase tracking-[0.16em]">
              {property.propertyType && (
                <span className="text-foreground/70">
                  {property.propertyType}
                </span>
              )}

              {property.standardStatus && (
                <span className="border-l border-border pl-3 text-accent">
                  {property.standardStatus}
                </span>
              )}
            </div>
          )}
        </div>

        <div className="flex flex-1 items-center">
          <PropertyFactsCard property={property} compact />
        </div>

        <div className="border-t border-border/90 bg-muted/15 px-5 py-2.5 lg:px-6">
          <PropertyActions property={property} />
        </div>
      </div>
    </section>
  )
}
