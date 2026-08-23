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
  compact = false,
  children,
}: {
  eyebrow: string
  title: string
  subtitle?: string
  compact?: boolean
  children?: ReactNode
}) {
  return (
    <div className={compact ? "mb-4" : "mb-8"}>
      <div className="h-px w-9 bg-[var(--portal-gold)]" aria-hidden />

      <div
        className={`flex flex-wrap items-end justify-between gap-3 ${
          compact ? "mt-3" : "mt-4"
        }`}
      >
        <div className="min-w-0">
          <p className="text-[10px] font-light uppercase tracking-[0.22em] text-black/40">
            {eyebrow}
          </p>

          <h1
            className={`font-serif font-light leading-[1.1] ${
              compact ? "mt-1 text-2xl" : "mt-3 text-4xl"
            }`}
          >
            {title}
          </h1>

          {subtitle && !compact ? (
            <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
              {subtitle}
            </p>
          ) : null}
        </div>

        {children ? <div className="flex shrink-0 flex-wrap items-center gap-2">{children}</div> : null}
      </div>
    </div>
  )
}
