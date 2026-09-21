import type { Metadata } from 'next'
import { notFound } from 'next/navigation'

import { AppleMapTest } from '@/components/dev/apple-map-test'
import { SiteHeader } from '@/components/site-header'

export const dynamic = 'force-dynamic'

export const metadata: Metadata = {
  title: 'Apple MapKit JS Test — CulebraLuxe',
  robots: {
    index: false,
    follow: false,
  },
}

export default function AppleMapTestPage() {
  if (process.env.NODE_ENV !== 'development') {
    notFound()
  }

  const token = process.env.APPLE_MAPKIT_JS_TOKEN?.trim() || null

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
              Apple MapKit JS — Casa Luar
            </h1>

            <p className="mt-3 text-sm leading-relaxed text-brand-navy/75">
              Isolated coverage and interaction test for Culebra. This route does
              not replace or modify the production property map.
            </p>
          </div>

          <AppleMapTest token={token} />
        </div>
      </main>
    </>
  )
}
