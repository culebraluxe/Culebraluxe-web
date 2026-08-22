import type { ReactNode } from "react"

// ---------------------------------------------------------------------------
// PORTAL-04 — one canonical Portal page head: gold hairline, eyebrow, serif
// title, optional subtitle and an optional trailing slot (meta rows, actions).
//
// Presentation-only. Geometry is preserved exactly from the inline page heads
// it replaces (eyebrow / H1 / subtitle spacing unchanged); the only new
// element is the short gold hairline above the eyebrow — the brand accent that
// lifts the headline hierarchy on the primary Portal surfaces. Trailing
// children keep their own top margins (no wrapper), so detail-page meta rows
// and back links render at their established spacing.
// ---------------------------------------------------------------------------

export function PageHeader({
  eyebrow,
  title,
  subtitle,
  children,
}: {
  eyebrow: string
  title: string
  subtitle?: string
  children?: ReactNode
}) {
  return (
    <div className="mb-8">
      <div className="h-px w-9 bg-[#c6a15b]" aria-hidden />

      <p className="mt-4 text-xs font-light uppercase tracking-[0.28em] text-black/40">
        {eyebrow}
      </p>

      <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
        {title}
      </h1>

      {subtitle ? (
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          {subtitle}
        </p>
      ) : null}

      {children}
    </div>
  )
}
