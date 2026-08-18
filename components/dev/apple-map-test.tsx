'use client'

import { useEffect, useRef, useState } from 'react'

const CASA_LUAR_COORDINATE = {
  latitude: 18.315573,
  longitude: -65.25565,
}

const MAPKIT_SCRIPT_ID = 'culebraluxe-apple-mapkit-js'
const MAPKIT_CALLBACK = '__culebraLuxeAppleMapKitReady'

type MapKitMap = {
  addAnnotation: (annotation: unknown) => void
  destroy: () => void
}

type MapKitNamespace = {
  Map: new (
    element: HTMLElement,
    options: Record<string, unknown>,
  ) => MapKitMap
  MarkerAnnotation: new (
    coordinate: typeof CASA_LUAR_COORDINATE,
    options: Record<string, unknown>,
  ) => unknown
  MapType: {
    Standard: string
    Satellite: string
    Hybrid: string
  }
}

type MapKitWindow = Window & {
  mapkit?: MapKitNamespace
  __culebraLuxeAppleMapKitReady?: () => void
}

export function AppleMapTest({ token }: { token: string | null }) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<'waiting' | 'loading' | 'ready' | 'error'>(
    token ? 'loading' : 'waiting',
  )

  useEffect(() => {
    if (!token || !containerRef.current) return

    const mapKitWindow = window as MapKitWindow
    let map: MapKitMap | null = null
    let disposed = false

    function initializeMap() {
      if (disposed || !containerRef.current || !mapKitWindow.mapkit) return

      try {
        const mapkit = mapKitWindow.mapkit
        map = new mapkit.Map(containerRef.current, {
          center: CASA_LUAR_COORDINATE,
          cameraDistance: 5200,
          mapType: mapkit.MapType.Standard,
          showsMapTypeControl: true,
          showsZoomControl: true,
          isScrollEnabled: true,
          isZoomEnabled: true,
          tintColor: '#c6a15b',
        })

        const annotation = new mapkit.MarkerAnnotation(CASA_LUAR_COORDINATE, {
          title: 'Casa Luar',
          subtitle: 'Zoni Estates, Culebra',
          accessibilityLabel: 'Casa Luar property location',
          color: '#030f23',
          glyphColor: '#c6a15b',
          glyphText: 'CL',
        })

        map.addAnnotation(annotation)
        setStatus('ready')
      } catch {
        setStatus('error')
      }
    }

    mapKitWindow[MAPKIT_CALLBACK] = initializeMap

    const existingScript = document.getElementById(MAPKIT_SCRIPT_ID)

    if (mapKitWindow.mapkit) {
      initializeMap()
    } else if (!existingScript) {
      const script = document.createElement('script')
      script.id = MAPKIT_SCRIPT_ID
      script.src = 'https://cdn.apple-mapkit.com/mk/x/mapkit.core.js'
      script.crossOrigin = 'anonymous'
      script.async = true
      script.dataset.callback = MAPKIT_CALLBACK
      script.dataset.libraries = 'full-map,annotations'
      script.dataset.token = token
      script.addEventListener('error', () => setStatus('error'), { once: true })
      document.head.appendChild(script)
    }

    return () => {
      disposed = true
      map?.destroy()
      delete mapKitWindow[MAPKIT_CALLBACK]
    }
  }, [token])

  return (
    <section aria-labelledby="apple-map-test-heading">
      <h2 id="apple-map-test-heading" className="sr-only">
        Casa Luar Apple map test
      </h2>

      <div className="relative h-[320px] w-full overflow-hidden rounded-sm border border-brand-navy/45 bg-brand-navy/[0.04] shadow-[0_10px_28px_rgba(3,15,35,0.06)] sm:h-[360px] lg:h-[450px]">
        <div ref={containerRef} className="h-full w-full" />

        {status !== 'ready' && (
          <div className="absolute inset-0 flex items-center justify-center bg-[#f5f2ec] px-6 text-center">
            <div className="max-w-lg">
              <p className="font-serif text-xl font-semibold text-brand-navy">
                {status === 'waiting'
                  ? 'MapKit JS token required'
                  : status === 'error'
                    ? 'MapKit JS could not initialize'
                    : 'Loading Apple Maps'}
              </p>

              <p className="mt-3 text-sm leading-relaxed text-brand-navy/75">
                {status === 'waiting'
                  ? 'Add a development MapKit JS token to APPLE_MAPKIT_JS_TOKEN in .env.local, then restart the local development server.'
                  : status === 'error'
                    ? 'Confirm that the token is valid for MapKit JS and permits the current localhost origin.'
                    : 'Loading standard and satellite map coverage for Culebra.'}
              </p>
            </div>
          </div>
        )}
      </div>

      <div className="mt-4 flex flex-col gap-2 border-t border-brand-navy/25 pt-4 text-sm text-brand-navy/80 sm:flex-row sm:items-center sm:justify-between">
        <p>
          <span className="font-semibold text-brand-navy">Casa Luar</span>
          {' · '}Zoni Estates, Culebra
        </p>

        <p className="font-mono text-xs tabular-nums text-brand-navy/70">
          18.315573, -65.255650
        </p>
      </div>

      <p className="mt-3 text-xs leading-relaxed text-brand-navy/55">
        Apple’s native map-type control provides Standard, Satellite, and Hybrid
        views when MapKit JS initializes.
      </p>
    </section>
  )
}
