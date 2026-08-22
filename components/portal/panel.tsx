import type { ReactNode } from 'react'

// ---------------------------------------------------------------------------
// Shared Portal panel primitive (visual backbone).
//
// One panel component + shared tokens for the whole portal. Variants:
//   standard  — white surface, confident cool-gray/navy border, restrained
//               shadow (default).
//   soft      — pale cool blue-gray surface (breaks up white space).
//   feature   — deep navy surface, white text, restrained gold accents; used
//               sparingly for high-priority information.
//   attention — pale cool surface, navy boundary, restrained gold eyebrow —
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
  standard:
    'border-[var(--portal-panel-border)] bg-[var(--portal-panel-bg)] shadow-[var(--portal-panel-shadow)]',
  soft: 'border-[var(--portal-panel-border)] [background:var(--portal-soft-gradient)] shadow-[var(--portal-panel-shadow)]',
  feature:
    'border-[var(--portal-feature-border)] [background:var(--portal-feature-gradient)] shadow-[var(--portal-feature-shadow)] text-white',
  attention:
    'border-[var(--portal-attention-border)] border-t-2 border-t-[var(--portal-gold)] bg-[var(--portal-attention-bg)] shadow-[var(--portal-panel-shadow)]',
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
  className?: string
  children?: ReactNode
}) {
  const hasHeader = Boolean(eyebrow || heading || subtitle || action)

  return (
    <section
      className={[
        'overflow-hidden rounded-[var(--portal-panel-radius)] border',
        flush ? '' : 'p-[var(--portal-panel-padding)]',
        variantSurface[variant],
        className,
      ].join(' ')}
    >
      {hasHeader ? (
        <header
          className={[
            'flex flex-wrap items-start justify-between gap-4',
            flush ? 'px-[var(--portal-panel-padding)] py-5' : 'mb-5',
            divider ? 'border-b border-[var(--portal-panel-border)] pb-5' : '',
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
                className={`mt-1.5 font-serif text-2xl font-light ${headingTone[variant]}`}
              >
                {heading}
              </h2>
            ) : null}
            {subtitle ? (
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
