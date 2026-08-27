'use client'

import { useEffect, useRef, useState } from 'react'

import {
  loadGoogleMaps,
  type GoogleMapInstance,
} from '@/lib/google-maps-loader'

// AdvancedMarkerElement requires a mapId on the map instance. This mirrors the
// established Google demo map id used by the dev spike. A dedicated production
// map id can be swapped in here later without touching marker semantics.
const PROPERTY_MAP_ID = 'DEMO_MAP_ID'

export function GooglePropertyMap({
  apiKey,
  latitude,
  longitude,
  title,
}: {
  apiKey: string | null
  latitude: number
  longitude: number
  title: string
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<'waiting' | 'loading' | 'ready' | 'error'>(
    apiKey ? 'loading' : 'waiting',
  )

  useEffect(() => {
    if (!apiKey || !containerRef.current) return

    let disposed = false
    let map: GoogleMapInstance | null = null
    let marker: (HTMLElement & { map: GoogleMapInstance | null }) | null = null
    let clearMapListeners: ((instance: object) => void) | null = null

    loadGoogleMaps(apiKey)
      .then((maps) => {
        if (disposed || !containerRef.current) return

        const coordinate = { lat: latitude, lng: longitude }

        map = new maps.Map(containerRef.current, {
          center: coordinate,
          zoom: 14,
          mapId: PROPERTY_MAP_ID,
          mapTypeId: maps.MapTypeId.ROADMAP,
          mapTypeControl: true,
          mapTypeControlOptions: {
            style: maps.MapTypeControlStyle.DROPDOWN_MENU,
            mapTypeIds: [
              maps.MapTypeId.ROADMAP,
              maps.MapTypeId.SATELLITE,
              maps.MapTypeId.HYBRID,
            ],
          },
          zoomControl: true,
          cameraControl: true,
          streetViewControl: false,
          fullscreenControl: true,
          clickableIcons: false,
          gestureHandling: 'auto',
        })
        clearMapListeners = maps.event.clearInstanceListeners

        const markerSvg = `
          <svg xmlns="http://www.w3.org/2000/svg" width="44" height="52" viewBox="0 0 44 52">
            <path fill="#030f23" stroke="#c6a15b" stroke-width="2" d="M22 1C10.4 1 1 10.4 1 22c0 15.1 21 29 21 29s21-13.9 21-29C43 10.4 33.6 1 22 1Z"/>
            <circle cx="22" cy="21" r="12" fill="#030f23" stroke="#c6a15b" stroke-width="1"/>
            <text x="22" y="25" text-anchor="middle" fill="#c6a15b" font-family="Arial, sans-serif" font-size="10" font-weight="700">CL</text>
          </svg>
        `.trim()

        // Advanced markers centre the content element on the coordinate; the SVG
        // is 44x52 and its pin tip is at the bottom edge (the legacy anchor was
        // 22,52). translateY(-26px) lifts it half its height so the tip still
        // rests exactly on the property coordinate (same visual intent).
        const markerContent = document.createElement('div')
        markerContent.style.cssText = 'transform: translateY(-26px);'
        markerContent.innerHTML = markerSvg

        marker = new maps.marker.AdvancedMarkerElement({
          map,
          position: coordinate,
          title: `${title} property location`,
          content: markerContent,
          gmpClickable: true,
        })

        setStatus('ready')
      })
      .catch(() => {
        if (!disposed) setStatus('error')
      })

    return () => {
      disposed = true
      if (marker) marker.map = null

      if (map) clearMapListeners?.(map)
    }
  }, [apiKey, latitude, longitude, title])

  return (
    <div className="relative h-full w-full bg-brand-navy/[0.04]">
      <div
        ref={containerRef}
        className="h-full w-full"
        aria-label={`Google map of ${title}`}
      />

      {status !== 'ready' && (
        <div className="absolute inset-0 flex items-center justify-center bg-[#f5f2ec] px-8 text-center">
          <div className="max-w-sm">
            <p className="font-serif text-xl font-semibold text-brand-navy">
              {status === 'waiting'
                ? 'Map temporarily unavailable'
                : status === 'error'
                  ? 'Map could not be loaded'
                  : 'Loading map'}
            </p>

            <p className="mt-3 text-sm font-normal leading-relaxed text-brand-navy/82">
              {status === 'waiting'
                ? 'Precise location information is available through CulebraLuxe.'
                : status === 'error'
                  ? 'Please contact CulebraLuxe for precise location information.'
                  : 'Loading the property location.'}
            </p>
          </div>
        </div>
      )}
    </div>
  )
}
