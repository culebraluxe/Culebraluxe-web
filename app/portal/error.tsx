'use client'

import Link from 'next/link'

// ---------------------------------------------------------------------------
// AUTH-BOUNDARY — Portal-specific controlled failure boundary.
//
// This is the ONLY error boundary for /portal/*. If Portal authentication (or
// any Portal dependency) fails — Auth.js config, database identity lookup,
// etc. — the failure is contained HERE and shown as a calm "Portal
// temporarily unavailable" screen. It never bubbles to the public site's error
// page, never shows a stack trace, and never exposes secret/config details.
// The public CulebraLuxe website is unaffected.
//
// Must remain auth-free and presentation-only.
// ---------------------------------------------------------------------------

export default function PortalError({
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  return (
    <main className="flex min-h-[80svh] items-center justify-center bg-[var(--portal-navy)] px-6 md:px-12">
      <div className="mx-auto w-full max-w-xl text-center">
        <p className="mb-6 text-xs font-light uppercase tracking-[0.4em] text-[var(--portal-gold)]">
          CulebraLuxe Portal
        </p>
        <h1 className="text-balance font-serif text-3xl font-light leading-[1.1] text-white md:text-4xl">
          Portal temporarily unavailable.
        </h1>
        <p className="mx-auto mt-6 max-w-md text-pretty text-sm font-light leading-relaxed text-white/70">
          The Portal could not be reached right now. This does not affect the
          public CulebraLuxe website. Please try again in a moment, or return
          home.
        </p>

        <div className="mt-12 flex flex-wrap items-center justify-center gap-4">
          <button
            type="button"
            onClick={reset}
            className="inline-flex min-h-11 items-center gap-3 border border-white/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-white transition-colors duration-500 hover:border-white"
          >
            Try again
          </button>
          <Link
            href="/"
            className="inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-white/80 transition-colors hover:text-white"
          >
            Return home
          </Link>
        </div>
      </div>
    </main>
  )
}
