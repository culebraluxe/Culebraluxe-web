// Reusable CulebraLuxe progress primitives (PORTAL-13 visual pass).
// Theme-token-driven, tiny, no charting dependency. Suitable for the UI Lab.

export function PortalProgressBar({
  value,
  tone = "dark",
  showValue = false,
  className = "",
}: {
  value: number
  tone?: "light" | "dark"
  showValue?: boolean
  className?: string
}) {
  const pct = Math.max(0, Math.min(100, value))
  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <div
        className={`h-[3px] flex-1 overflow-hidden rounded-full ${
          tone === "dark" ? "bg-white/15" : "bg-black/10"
        }`}
      >
        <div
          className={`h-full rounded-full ${
            tone === "dark" ? "bg-[var(--portal-gold)]/80" : "bg-[var(--portal-navy)]"
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
      {showValue && (
        <span
          className={`text-[10px] font-light tabular-nums ${
            tone === "dark" ? "text-white/55" : "text-black/50"
          }`}
        >
          {Math.round(pct)}%
        </span>
      )}
    </div>
  )
}

export function PortalProgressRing({
  value,
  size = 48,
  stroke = 4,
  tone = "dark",
}: {
  value: number
  size?: number
  stroke?: number
  tone?: "light" | "dark"
}) {
  const pct = Math.max(0, Math.min(100, value))
  const r = (size - stroke) / 2
  const c = 2 * Math.PI * r
  const offset = c * (1 - pct / 100)
  const color = tone === "dark" ? "var(--portal-gold)" : "var(--portal-navy)"
  const track = tone === "dark" ? "rgba(255,255,255,0.12)" : "rgba(24,43,64,0.12)"
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={track}
          strokeWidth={stroke}
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={offset}
        />
      </svg>
      <div className="absolute inset-0 flex items-center justify-center">
        <span
          className={`font-serif text-sm font-light tabular-nums ${
            tone === "dark" ? "text-white" : "text-[var(--portal-navy)]"
          }`}
        >
          {Math.round(pct)}%
        </span>
      </div>
    </div>
  )
}
