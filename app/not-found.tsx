import Link from 'next/link'

import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'

export default function NotFound() {
  return (
    <>
      <SiteHeader />
      <main className="bg-foreground px-6 py-32 text-background md:px-12 md:py-44">
        <div className="mx-auto max-w-[1600px]">
          <p className="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/60">
            CulebraLuxe
          </p>
          <h1 className="text-balance font-serif text-5xl font-light leading-[1.02] text-background md:text-7xl">
            404
          </h1>
          <p className="mt-6 max-w-xl text-pretty text-base font-light leading-relaxed text-background/75">
            This page has drifted out to sea. The address may have changed, or
            the page may no longer exist.
          </p>

          <div className="mt-12 flex flex-wrap items-center gap-x-10 gap-y-4">
            <Link
              href="/"
              className="group inline-flex items-center gap-3 border border-background/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-background transition-colors duration-500 hover:border-background"
            >
              Return home
              <span className="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
            </Link>
            <Link
              href="/buyers"
              className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-background/80 transition-colors hover:text-background"
            >
              Explore properties
              <span className="inline-block h-px w-8 bg-background/60 transition-all duration-500 group-hover:w-14" />
            </Link>
          </div>
        </div>
      </main>
      <SiteFooter />
    </>
  )
}
