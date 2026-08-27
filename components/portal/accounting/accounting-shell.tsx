import type { PnlTrendPoint } from '@/db/accounting'
import { formatMoney } from '@/lib/accounting/format'

// ---------------------------------------------------------------------------
// ACCOUNTING V1 — shared dark navy/gold surfaces for the accounting module.
// Reuses existing Design Lab tokens (panel radius, brand navy, brand gold).
// ---------------------------------------------------------------------------

export function AccountingShell({
  eyebrow,
  title,
  children,
}: {
  eyebrow?: string
  title?: string
  children: React.ReactNode
}) {
  return (
    <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)] p-4 text-white shadow-[0_24px_70px_rgba(3,15,35,0.45)] sm:p-5 lg:p-6">
      {(eyebrow || title) && (
        <div className="mb-5">
          {eyebrow && (
            <p className="text-[10px] font-medium uppercase tracking-[0.24em] text-[var(--portal-gold)]">
              {eyebrow}
            </p>
          )}
          {title && (
            <h1 className="mt-1 font-serif text-2xl font-light leading-tight text-white">
              {title}
            </h1>
          )}
        </div>
      )}
      {children}
    </div>
  )
}

export function MetricCard({
  label,
  value,
  hint,
  tone = 'neutral',
}: {
  label: string
  value: string
  hint?: string
  tone?: 'neutral' | 'good' | 'warn' | 'bad'
}) {
  const toneClass =
    tone === 'good'
      ? 'text-emerald-300'
      : tone === 'warn'
        ? 'text-amber-300'
        : tone === 'bad'
          ? 'text-rose-300'
          : 'text-white'
  return (
    <div className="rounded-[var(--portal-panel-radius)] border border-white/10 bg-gradient-to-b from-white/[0.09] to-white/[0.03] p-4">
      <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">
        {label}
      </p>
      <p className={`mt-2 font-serif text-3xl font-light ${toneClass}`}>{value}</p>
      {hint && <p className="mt-1 text-[11px] font-light text-white/50">{hint}</p>}
    </div>
  )
}

export function GlassPanel({
  title,
  action,
  children,
  className = '',
}: {
  title?: string
  action?: React.ReactNode
  children: React.ReactNode
  className?: string
}) {
  return (
    <section
      className={`rounded-[var(--portal-panel-radius)] border border-white/10 bg-gradient-to-b from-white/[0.06] to-white/[0.02] ${className}`}
    >
      {(title || action) && (
        <div className="flex items-center justify-between gap-3 border-b border-white/10 px-4 py-3">
          {title && (
            <h2 className="text-[11px] font-medium uppercase tracking-[0.18em] text-white/70">
              {title}
            </h2>
          )}
          {action}
        </div>
      )}
      <div className="p-4">{children}</div>
    </section>
  )
}

// --- P&L trend line chart (dependency-free SVG) ----------------------------

export function PnlTrendChart({ data }: { data: PnlTrendPoint[] }) {
  const width = 640
  const height = 120
  const pad = 18

  const values = data.flatMap((d) => [d.income, d.expenses, d.net])
  const maxAbs = Math.max(1, ...values.map((v) => Math.abs(v)))
  const maxVal = maxAbs

  const x = (i: number) =>
    pad + (i * (width - pad * 2)) / Math.max(1, data.length - 1)
  const y = (v: number) =>
    pad + ((maxVal - v) / (maxVal * 2)) * (height - pad * 2)

  const series: Array<{ key: 'income' | 'expenses' | 'net'; color: string }> = [
    { key: 'income', color: '#c6a15b' },
    { key: 'expenses', color: '#7dd3fc' },
    { key: 'net', color: '#ffffff' },
  ]

  return (
    <div>
      <svg viewBox={`0 0 ${width} ${height}`} className="h-auto w-full">
        <line
          x1={pad}
          x2={width - pad}
          y1={height / 2}
          y2={height / 2}
          stroke="rgba(255,255,255,0.12)"
          strokeDasharray="3 4"
        />
        {data.map((d, i) => (
          <text
            key={d.month}
            x={x(i)}
            y={height - 6}
            textAnchor="middle"
            className="fill-white/40"
            style={{ fontSize: 9 }}
          >
            {d.month}
          </text>
        ))}
        {series.map(({ key, color }) => {
          const points = data
            .map((d, i) => `${x(i)},${y(d[key])}`)
            .join(' ')
          return (
            <polyline
              key={key}
              points={points}
              fill="none"
              stroke={color}
              strokeWidth={key === 'net' ? 2.5 : 1.5}
              strokeLinejoin="round"
              strokeLinecap="round"
              opacity={key === 'net' ? 1 : 0.55}
            />
          )
        })}
      </svg>
      <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[10px] font-light uppercase tracking-[0.16em] text-white/50">
        <span className="flex items-center gap-1.5">
          <span className="h-1.5 w-3 rounded-full bg-[#c6a15b]" /> Income
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-1.5 w-3 rounded-full bg-sky-300" /> Expenses
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-1.5 w-3 rounded-full bg-white" /> Net
        </span>
        <span className="ml-auto text-white/60">{formatMoney(data[0]?.net ?? 0)}</span>
      </div>
    </div>
  )
}
