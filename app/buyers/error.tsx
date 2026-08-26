'use client'

import Link from 'next/link'

// DB-HARDEN-01 — route-local degradation for the Buyers collection. If the
// inventory query fails (DB unavailable / schema mismatch), the buyers route
// shows a controlled "Listings temporarily unavailable" state instead of
// crashing or surfacing a stack trace. Unrelated routes are unaffected.
export default function BuyersError({
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  return (
    <main className="flex min-h-[70svh] items-center bg-[#f5f2ec] px-6 md:px-12">
      <div className="mx-auto w-full max-w-2xl">
        <p className="mb-6 text-xs font-light uppercase tracking-[0.4em] text-[#030f23]/50">
          CulebraLuxe
        </p>
        <h1 className="text-balance font-serif text-4xl font-light leading-[1.05] text-[#030f23]">
          Listings temporarily unavailable.
        </h1>
        <p className="mt-6 max-w-xl text-pretty text-sm font-light leading-relaxed text-[#030f23]/60">
          We could not load the current listings. Please try again in a moment.
        </p>
        <div className="mt-12 flex flex-wrap items-center gap-6">
          <button
            type="button"
            onClick={reset}
            className="inline-flex min-h-11 items-center border border-[#030f23]/30 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-[#030f23] transition hover:border-[#030f23]"
          >
            Try again
          </button>
          <Link
            href="/"
            className="inline-flex items-center text-xs font-light uppercase tracking-[0.22em] text-[#030f23]/70 transition hover:text-[#030f23]"
          >
            Return home
          </Link>
        </div>
      </div>
    </main>
  )
}
