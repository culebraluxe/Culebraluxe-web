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
    <section className="overflow-hidden border border-brand-navy/20 bg-card shadow-[0_14px_36px_rgba(3,15,35,0.06)] lg:grid lg:min-h-[432px] lg:grid-cols-[minmax(0,1.5fr)_minmax(380px,1fr)]">
      <div className="relative h-[290px] sm:h-[370px] lg:h-[432px]">
        <PropertyMediaPanel
          heroUrl={heroUrl}
          galleryImages={galleryImages}
          propertyTitle={property.title}
        />
      </div>

      <div className="flex flex-col border-l-0 border-brand-navy/15 bg-card lg:border-l">
        <div className="border-b border-brand-navy/15 px-5 py-3.5 lg:px-6">
          <div className="mb-2 flex items-center gap-2">
            {locationLine && (
              <p className="flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.18em] text-brand-navy/65">
                <MapPin className="h-3.5 w-3.5 text-brand-gold" aria-hidden />
                {locationLine}
              </p>
            )}
          </div>

          <h1 className="text-balance font-serif text-2xl font-medium leading-tight text-brand-navy xl:text-3xl">
            {property.title}
          </h1>

          <p className="mt-1.5 text-lg font-medium text-foreground/90">
            {formatPrice(property.listPrice)}
          </p>

          {(property.propertyType || property.standardStatus) && (
            <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] font-medium uppercase tracking-[0.16em]">
              {property.propertyType && (
                <span className="text-brand-navy/70">
                  {property.propertyType}
                </span>
              )}

              {property.standardStatus && (
                <span className="border-l border-brand-gold/40 pl-3 text-brand-gold">
                  {property.standardStatus}
                </span>
              )}
            </div>
          )}
        </div>

        <div className="flex flex-1 items-center">
          <PropertyFactsCard property={property} compact />
        </div>

        <div className="border-t border-brand-navy/15 bg-brand-navy/[0.025] px-5 py-2.5 lg:px-6">
          <PropertyActions property={property} />
        </div>
      </div>
    </section>
  )
}
