import type { PropertyDetail } from '@/lib/property-types'

export function PropertyLocation({ property }: { property: PropertyDetail }) {
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
        <div className="h-[320px] w-full overflow-hidden rounded-sm border border-border/90 sm:h-[360px] lg:h-[450px]">
          <iframe
            title={`Map of ${property.title ?? 'property'}`}
            className="h-full w-full"
            loading="lazy"
            referrerPolicy="no-referrer-when-downgrade"
            src={`https://www.openstreetmap.org/export/embed.html?bbox=${
              longitude! - 0.012
            }%2C${latitude! - 0.008}%2C${
              longitude! + 0.012
            }%2C${latitude! + 0.008}&layer=mapnik&marker=${
              latitude
            }%2C${longitude}`}
          />
        </div>
      ) : (
        <div className="flex h-[300px] items-center justify-center rounded-sm border border-border/90 bg-muted/10 px-8 text-center sm:h-[340px] lg:h-[450px]">
          <div className="max-w-sm">
            <p className="font-serif text-xl font-light text-foreground">
              Private Location
            </p>

            <p className="mt-3 text-sm font-light leading-relaxed text-muted-foreground">
              Precise location information is available through CulebraLuxe.
            </p>
          </div>
        </div>
      )}

      {hasLocationContext && (
        <aside className="border-t border-border/90 bg-muted/10 px-5 py-5 sm:px-6 sm:py-6">
          <p className="text-xs font-light uppercase tracking-[0.24em] text-accent">
            Location
          </p>

          <h2 className="mt-4 font-serif text-2xl font-light leading-tight text-foreground">
            {property.neighborhood ?? property.city}
          </h2>

          {property.neighborhood && broaderLocation && (
            <p className="mt-1.5 text-sm font-light text-muted-foreground">
              {broaderLocation}
            </p>
          )}

          <div className="mt-5 border-t border-border/80 pt-5">
            <p className="text-sm font-light leading-relaxed text-foreground/80">
              This property is located in {locationLine}.
            </p>
          </div>

          {(property.neighborhood || property.city || property.stateOrProvince) && (
            <dl className="mt-6 space-y-3">
              {property.neighborhood && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-light uppercase tracking-[0.16em] text-muted-foreground">
                    Neighborhood
                  </dt>

                  <dd className="text-right text-sm font-normal text-foreground/90">
                    {property.neighborhood}
                  </dd>
                </div>
              )}

              {property.city && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-light uppercase tracking-[0.16em] text-muted-foreground">
                    Municipality
                  </dt>

                  <dd className="text-right text-sm font-normal text-foreground/90">
                    {property.city}
                  </dd>
                </div>
              )}

              {property.stateOrProvince && (
                <div className="flex items-baseline justify-between gap-5">
                  <dt className="text-[10px] font-light uppercase tracking-[0.16em] text-muted-foreground">
                    Region
                  </dt>

                  <dd className="text-right text-sm font-normal text-foreground/90">
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
