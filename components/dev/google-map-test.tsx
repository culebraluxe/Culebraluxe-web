'use client'

import { useEffect, useRef, useState } from 'react'

import {
  loadGoogleMaps,
  type GoogleMapInstance,
} from '@/lib/google-maps-loader'

const CASA_LUAR_COORDINATE = {
  lat: 18.315573,
  lng: -65.25565,
}

type AdvancedMarker = HTMLElement & {
  map: GoogleMapInstance | null
}

export function GoogleMapTest({ apiKey }: { apiKey: string | null }) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<'waiting' | 'loading' | 'ready' | 'error'>(
    apiKey ? 'loading' : 'waiting',
  )

  useEffect(() => {
    if (!apiKey || !containerRef.current) return

    let marker: AdvancedMarker | null = null
    let disposed = false

    loadGoogleMaps(apiKey)
      .then((maps) => {
        if (disposed || !containerRef.current) return

        const map = new maps.Map(containerRef.current, {
          center: CASA_LUAR_COORDINATE,
          zoom: 14,
          mapId: 'DEMO_MAP_ID',
          mapTypeId: maps.MapTypeId.ROADMAP,
          mapTypeControl: true,
          mapTypeControlOptions: {
            style: maps.MapTypeControlStyle.DROPDOWN_MENU,
            mapTypeIds: [
              maps.MapTypeId.ROADMAP,
              maps.MapTypeId.SATELLITE,
              maps.MapTypeId.HYBRID,
              maps.MapTypeId.TERRAIN,
            ],
          },
          zoomControl: true,
          cameraControl: true,
          streetViewControl: false,
          fullscreenControl: true,
          clickableIcons: false,
          gestureHandling: 'auto',
        })

        marker = new maps.marker.AdvancedMarkerElement({
          map,
          position: CASA_LUAR_COORDINATE,
          title: 'Casa Luar — Zoni Estates, Culebra',
          gmpClickable: true,
        })

        const pin = new maps.marker.PinElement({
          background: '#030f23',
          borderColor: '#c6a15b',
          glyphColor: '#c6a15b',
          glyphText: 'CL',
          scale: 0.95,
        })

        marker.append(pin)
        setStatus('ready')
      })
      .catch(() => {
        if (!disposed) setStatus('error')
      })

    return () => {
      disposed = true

      if (marker) {
        marker.map = null
      }
    }
  }, [apiKey])

  return (
    <section aria-labelledby="google-map-test-heading">
      <h2 id="google-map-test-heading" className="sr-only">
        Casa Luar Google map test
      </h2>

      <div className="relative h-[320px] w-full overflow-hidden rounded-sm border border-brand-navy/45 bg-brand-navy/[0.04] shadow-[0_10px_28px_rgba(3,15,35,0.06)] sm:h-[360px] lg:h-[450px]">
        <div ref={containerRef} className="h-full w-full" />

        {status !== 'ready' && (
          <div className="absolute inset-0 flex items-center justify-center bg-[#f5f2ec] px-6 text-center">
            <div className="max-w-lg">
              <p className="font-serif text-xl font-semibold text-brand-navy">
                {status === 'waiting'
                  ? 'Google Maps Demo Key required'
                  : status === 'error'
                    ? 'Google Maps could not initialize'
                    : 'Loading Google Maps'}
              </p>

              <p className="mt-3 text-sm leading-relaxed text-brand-navy/75">
                {status === 'waiting'
                  ? 'Add the prototype key to GOOGLE_MAPS_DEMO_KEY in .env.local, then restart the local development server.'
                  : status === 'error'
                    ? 'Confirm that the Demo Key is valid and available for the current localhost origin.'
                    : 'Loading roadmap and aerial coverage for Culebra.'}
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
        The map-type control exposes Roadmap, Satellite, Hybrid, and Terrain for
        direct coverage comparison.
      </p>
    </section>
  )
}
