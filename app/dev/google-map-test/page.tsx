import type { Metadata } from 'next'
import { notFound } from 'next/navigation'

import { GoogleMapTest } from '@/components/dev/google-map-test'
import { SiteHeader } from '@/components/site-header'

export const dynamic = 'force-dynamic'

export const metadata: Metadata = {
  title: 'Google Maps JavaScript Test — CulebraLuxe',
  robots: {
    index: false,
    follow: false,
  },
}

export default function GoogleMapTestPage() {
  if (process.env.NODE_ENV !== 'development') {
    notFound()
  }

  const apiKey = process.env.GOOGLE_MAPS_DEMO_KEY?.trim() || null

  return (
    <>
      <SiteHeader />

      <main className="min-h-[calc(100vh-77px)] bg-background">
        <div className="mx-auto max-w-[1280px] px-6 py-8 md:px-12 md:py-10">
          <div className="mb-7 max-w-3xl">
            <p className="text-xs font-medium uppercase tracking-[0.28em] text-[#c6a15b]">
              Development Spike
            </p>

            <h1 className="mt-3 font-serif text-3xl font-semibold text-brand-navy sm:text-4xl">
              Google Maps JavaScript — Casa Luar
            </h1>

            <p className="mt-3 text-sm leading-relaxed text-brand-navy/75">
              Isolated map-quality test for Culebra using Google’s development
              Demo Key path. This route does not modify the production map.
            </p>
          </div>

          <GoogleMapTest apiKey={apiKey} />
        </div>
      </main>
    </>
  )
}
