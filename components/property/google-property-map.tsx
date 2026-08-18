'use client'

import { useEffect, useRef, useState } from 'react'

import {
  loadGoogleMaps,
  type GoogleMapInstance,
} from '@/lib/google-maps-loader'

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
    let marker: { setMap: (map: GoogleMapInstance | null) => void } | null = null
    let clearMapListeners: ((instance: object) => void) | null = null

    loadGoogleMaps(apiKey)
      .then((maps) => {
        if (disposed || !containerRef.current) return

        const coordinate = { lat: latitude, lng: longitude }

        map = new maps.Map(containerRef.current, {
          center: coordinate,
          zoom: 14,
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

        marker = new maps.Marker({
          map,
          position: coordinate,
          title: `${title} property location`,
          icon: {
            url: `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(markerSvg)}`,
            scaledSize: new maps.Size(44, 52),
            anchor: new maps.Point(22, 52),
          },
        })

        setStatus('ready')
      })
      .catch(() => {
        if (!disposed) setStatus('error')
      })

    return () => {
      disposed = true
      marker?.setMap(null)

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
