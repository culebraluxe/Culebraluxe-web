import type { ReactNode } from 'react'

// ---------------------------------------------------------------------------
// Shared Portal panel primitive (visual backbone).
//
// One panel component + shared tokens for the whole portal. Variants:
//   standard  — frosted glass (default).
//   soft      — cooler, more translucent glass (breaks up stacked cards).
//   feature   — navy glass, white text, restrained gold accents; used
//               sparingly for high-priority information.
//   attention — frosted glass, navy boundary, gold top edge —
//               brand-distinct without warning-yellow.
//
// Every visual knob (bg / border / shadow / radius / padding / heading) is
// driven by the --portal-* tokens in globals.css: a theme change propagates to
// every panel without per-page painting.
//
// `flush` renders an edge-to-edge panel (header padded by the panel token,
// body keeps its own row rhythm) — for tables and row lists that span the
// panel.
// ---------------------------------------------------------------------------

export type PanelVariant = 'standard' | 'soft' | 'feature' | 'attention'

const variantSurface: Record<PanelVariant, string> = {
  standard: 'portal-glass-panel',
  soft: 'portal-glass-panel portal-glass-panel-soft',
  feature: 'portal-glass-panel portal-glass-panel-feature',
  attention: 'portal-glass-panel portal-glass-panel-attention',
}

const headingTone: Record<PanelVariant, string> = {
  standard: 'text-[var(--portal-panel-heading)]',
  soft: 'text-[var(--portal-soft-heading)]',
  feature: 'text-[var(--portal-feature-heading)]',
  attention: 'text-[var(--portal-attention-heading)]',
}

const mutedTone: Record<PanelVariant, string> = {
  standard: 'text-[var(--portal-panel-heading-muted)]',
  soft: 'text-[var(--portal-blue-gray)]',
  feature: 'text-[var(--portal-feature-eyebrow)]',
  attention: 'text-[var(--portal-attention-eyebrow)]',
}

export function Panel({
  variant = 'standard',
  eyebrow,
  heading,
  subtitle,
  action,
  divider = false,
  flush = false,
  compact = false,
  className = '',
  children,
}: {
  variant?: PanelVariant
  eyebrow?: ReactNode
  heading?: ReactNode
  subtitle?: ReactNode
  action?: ReactNode
  divider?: boolean
  flush?: boolean
  compact?: boolean
  className?: string
  children?: ReactNode
}) {
  const hasHeader = Boolean(eyebrow || heading || subtitle || action)

  return (
    <section
      className={[
        'overflow-hidden rounded-[var(--portal-panel-radius)]',
        flush ? '' : compact ? 'p-4' : 'p-[var(--portal-panel-padding)]',
        variantSurface[variant],
        className,
      ].join(' ')}
    >
      {hasHeader ? (
        <header
          className={[
            'flex flex-wrap items-center justify-between gap-2',
            flush ? (compact ? 'px-4 py-3' : 'px-[var(--portal-panel-padding)] py-5') : compact ? 'mb-3' : 'mb-5',
            divider
              ? compact
                ? 'border-b border-[var(--portal-panel-border)] pb-3'
                : 'border-b border-[var(--portal-panel-border)] pb-5'
              : '',
          ].join(' ')}
        >
          <div className="min-w-0">
            {eyebrow ? (
              <p
                className={`text-[10px] font-light uppercase tracking-[0.22em] ${mutedTone[variant]}`}
              >
                {eyebrow}
              </p>
            ) : null}
            {heading ? (
              <h2
                className={`${eyebrow ? 'mt-1' : ''} font-serif font-light ${
                  compact ? 'text-lg' : 'mt-1.5 text-2xl'
                } ${headingTone[variant]}`}
              >
                {heading}
              </h2>
            ) : null}
            {subtitle && !compact ? (
              <p
                className={`mt-1 text-sm font-light ${
                  variant === 'feature' ? 'text-white/55' : 'text-black/45'
                }`}
              >
                {subtitle}
              </p>
            ) : null}
          </div>
          {action ? (
            <div className="flex shrink-0 items-center gap-2">{action}</div>
          ) : null}
        </header>
      ) : null}
      {children}
    </section>
  )
}
