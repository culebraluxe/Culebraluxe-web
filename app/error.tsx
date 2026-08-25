'use client'

import Link from 'next/link'

export default function Error({
  reset,
}: {
  reset: () => void
}) {
  return (
    <main className="flex min-h-[80svh] items-center bg-foreground px-6 text-background md:px-12">
      <div className="mx-auto w-full max-w-[1600px]">
        <p className="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/60">
          CulebraLuxe
        </p>
        <h1 className="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
          Something drifted off course.
        </h1>
        <p className="mt-6 max-w-xl text-pretty text-sm font-light leading-relaxed text-background/75">
          A momentary interruption while we prepared this page. You can try
          again, or return to the CulebraLuxe home.
        </p>

        <div className="mt-12 flex flex-wrap items-center gap-x-10 gap-y-4">
          <button
            type="button"
            onClick={reset}
            className="group inline-flex min-h-11 items-center gap-3 border border-background/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-background transition-colors duration-500 hover:border-background"
          >
            Try again
            <span className="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
          </button>
          <Link
            href="/"
            className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-background/80 transition-colors hover:text-background"
          >
            Return home
            <span className="inline-block h-px w-8 bg-background/60 transition-all duration-500 group-hover:w-14" />
          </Link>
        </div>
      </div>
    </main>
  )
}
