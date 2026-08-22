import { GooglePropertyMap } from '@/components/property/google-property-map'
import type { PropertyDetail } from '@/lib/property-types'

export function PropertyLocation({
  property,
  googleMapsApiKey,
}: {
  property: PropertyDetail
  googleMapsApiKey?: string | null
}) {
  const latitude = property.latitude
  const longitude = property.longitude
  const hasCoordinates =
    latitude != null && longitude != null
  const locationLine = [
    property.neighborhood,
    property.city,
    property.stateOrProvince,
  ]
    .filter(Boolean)
    .join(', ')
  const broaderLocation = [
    property.city,
    property.stateOrProvince,
  ]
    .filter(Boolean)
    .join(', ')
  const hasLocationContext = Boolean(locationLine)

  return (
    <div
      className={
        hasLocationContext
          ? 'grid items-start gap-7 lg:grid-cols-[minmax(0,2fr)_minmax(280px,1fr)] lg:gap-9'
          : 'max-w-4xl'
      }
    >
      {hasCoordinates ? (
        <div className="h-[320px] w-full overflow-hidden rounded-sm border border-brand-navy/45 shadow-[0_10px_28px_rgba(3,15,35,0.06)] sm:h-[360px] lg:h-[450px]">
          <GooglePropertyMap
            apiKey={googleMapsApiKey ?? null}
            latitude={latitude}
            longitude={longitude}
            title={property.title ?? 'Property'}
          />
        </div>
      ) : (
        <div className="flex h-[300px] items-center justify-center rounded-sm border border-brand-navy/45 bg-brand-navy/[0.05] px-8 text-center sm:h-[340px] lg:h-[450px]">
          <div className="max-w-sm">
            <p className="font-serif text-xl font-semibold text-brand-navy">
              Private Location
            </p>

            <p className="mt-3 text-sm font-normal leading-relaxed text-brand-navy/82">
              Precise location information is available through CulebraLuxe.
            </p>
          </div>
        </div>
      )}

      {hasLocationContext && (
        <aside className="border border-brand-navy/40 bg-brand-navy/[0.05] px-5 py-5 sm:px-6 sm:py-6">
          <p className="text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">
            Location
          </p>

          <h2 className="mt-4 font-serif text-2xl font-semibold leading-tight text-brand-navy">
            {property.neighborhood ?? property.city}
          </h2>

          {property.neighborhood && broaderLocation && (
            <p className="mt-1.5 text-sm font-normal text-brand-navy/80">
              {broaderLocation}
            </p>
          )}

          <div className="mt-5 border-t border-brand-navy/30 pt-5">
            <p className="text-sm font-normal leading-relaxed text-brand-navy/90">
              This property is located in {locationLine}.
            </p>
          </div>

          {(property.neighborhood || property.city || property.stateOrProvince) && (
            <dl className="mt-6 space-y-3">
              {property.neighborhood && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">
                    Neighborhood
                  </dt>

                  <dd className="text-right text-sm font-medium text-brand-navy">
                    {property.neighborhood}
                  </dd>
                </div>
              )}

              {property.city && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">
                    Municipality
                  </dt>

                  <dd className="text-right text-sm font-medium text-brand-navy">
                    {property.city}
                  </dd>
                </div>
              )}

              {property.stateOrProvince && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">
                    Region
                  </dt>

                  <dd className="text-right text-sm font-medium text-brand-navy">
                    {property.stateOrProvince}
                  </dd>
                </div>
              )}
            </dl>
          )}
        </aside>
      )}
    </div>
  )
}
