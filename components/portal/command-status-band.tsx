import type { ReactNode } from "react"

// ---------------------------------------------------------------------------
// COMMAND + STATUS BAND — reusable design-system component.
//
// PURPOSE
//   Provide a consistent top-level interaction pattern: a command / AI entry
//   surface (command slot) paired with concise system state (status slot), then
//   the primary page workspace below.
//
// USE WHEN
//   The page has a meaningful command/AI/orchestration action plus state/result
//   to communicate (e.g. Forms: Grok prompt + status).
//
// DO NOT USE WHEN
//   It would merely duplicate page navigation or create status chrome with no
//   meaningful function.
//
// LAYOUT
//   preset ratios on desktop/tablet, stacked (command above status) on mobile.
//   The component owns the responsive behaviour, glass/panel treatment,
//   spacing, typography hierarchy, status-dot placement, and accessibility.
//   Pages never reimplement breakpoints themselves.
//
// API
//   <CommandStatusBand ratio="wide-command" command={...} status={...} />
//   <CommandStatus label="Status" tone="success">…</CommandStatus>
// ---------------------------------------------------------------------------

export type CommandStatusBandRatio = "wide-command" | "balanced" | "wide-status" | "command-6040"

// Constrained layout presets (CSS grid fr values). No arbitrary per-page CSS.
const RATIO_GRID: Record<CommandStatusBandRatio, string> = {
  "wide-command": "lg:grid-cols-[minmax(0,1.9fr)_minmax(0,1fr)]", // ~65 / 35
  balanced: "lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]", // 50 / 50
  "wide-status": "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)]", // ~40 / 60
  // 60 / 40 — aligns the Command | Status seam with a 20 / 40 / 40 lower pane
  // seam (Command spans Navigator + Task; Status aligns over Calendar).
  "command-6040": "lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]",
}

export function CommandStatusBand({
  ratio = "wide-command",
  command,
  status,
}: {
  ratio?: CommandStatusBandRatio
  command: ReactNode
  status: ReactNode
}) {
  return (
    <div className={`grid grid-cols-1 gap-3 lg:gap-4 ${RATIO_GRID[ratio]}`}>
      <div className="min-w-0">{command}</div>
      <div className="min-w-0">{status}</div>
    </div>
  )
}

export type CommandStatusTone = "neutral" | "success" | "warning" | "danger"

const TONE_DOT: Record<CommandStatusTone, string> = {
  neutral: "bg-black/25",
  success: "bg-[var(--portal-success)]",
  warning: "bg-[var(--portal-gold)]",
  danger: "bg-[var(--portal-archive)]",
}

export function CommandStatus({
  label = "Status",
  tone = "neutral",
  children,
}: {
  label?: string
  tone?: CommandStatusTone
  children: ReactNode
}) {
  return (
    <section
      aria-label={label}
      className="portal-glass-panel portal-glass-panel-lifted flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]"
    >
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
        <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
          {label}
        </p>
        {/* Status is never communicated by colour alone: the text carries meaning. */}
        <span aria-hidden className={["h-2 w-2 shrink-0 rounded-full", TONE_DOT[tone]].join(" ")} />
      </div>
      <p
        aria-live="polite"
        className="min-h-0 flex-1 overflow-hidden px-4 py-2.5 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] line-clamp-3"
      >
        {children}
      </p>
    </section>
  )
}
