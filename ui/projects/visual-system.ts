// PROJECTS-WORKSPACE-02 — frozen two-pane geometry + visual system contract.
//
// This module is PURE: it imports no React and touches no DOM. It is the single
// source of truth for the approved desktop geometry (navy project navigator +
// dominant project canvas) and for the token-driven primitives the workspace
// consumes. Colors reference existing `--portal-*` custom properties only —
// never a raw hex. The selected-work editor now lives INSIDE the canvas as a
// compact subordinate control surface; it is no longer a third grid pane.
//
// The machine assay in `testv2/projects-workspace-02-tokens.test.ts` locks these
// invariants; the pixel-level balance is adjudicated by the HUMAN GATE.

export type ProjectsPaneRole = "navigator" | "canvas"

/** Permanent desktop role order is frozen: navigator -> canvas. */
export const PROJECTS_PANE_ORDER = ["navigator", "canvas"] as const

export type ProjectsSurfaceFamily = "navy" | "canvas" | "ivory"

export type ProjectsPaneGeometry = {
  readonly min: number
  readonly max: number | null
  readonly track: string
}

/**
 * Approved desktop geometry. `canvas` owns every remaining horizontal pixel
 * after the bounded navigator. Selected Work is stacked inside the canvas, so
 * Timeline / Calendar / Documents can use the width previously lost to the
 * inspector column. The viewport height is derived from a shell-owned token so
 * the workspace never hardcodes a magic `calc(100dvh - 10.5rem)`.
 */
export const PROJECTS_GEOMETRY = {
  breakpoint: "lg",
  gap: "0.75rem",
  gridClassName: "projects-workspace-grid grid min-h-0 flex-1 gap-3",
  viewport: {
    unit: "dvh",
    chromeHeightVar: "--portal-shell-chrome-height",
    fallback: "10.5rem",
  },
  columns: {
    navigator: { min: 350, max: 375, track: "minmax(350px,375px)" },
    canvas: { min: 0, max: null, track: "minmax(0,1fr)" },
  },
} as const

/** `grid-template-columns` value, derived so order and tracks cannot drift. */
export const PROJECTS_GRID_TEMPLATE = PROJECTS_PANE_ORDER.map(
  (role) => PROJECTS_GEOMETRY.columns[role].track,
).join(" ")

const PANE_BASE =
  "portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]"

/**
 * Permanent pane surfaces are navigator + canvas. `inspector` remains as a
 * compatibility surface for the old internal inspector implementation while
 * the two-pane layout settles; it is deliberately NOT present in
 * PROJECTS_PANE_ORDER or PROJECTS_GEOMETRY.columns and therefore owns no grid
 * track. Selected Work uses the cool portal soft surface in the canvas.
 *
 * IMPORTANT — this object is NOT what paints the permanent panes. The paint
 * comes from the `.projects-pane-*` rules in app/globals.css; the active roles
 * are mirrored in tests so those two cannot drift again.
 */
export const PROJECTS_SURFACE = {
  navigator: {
    family: "navy",
    className: `${PANE_BASE} projects-pane projects-pane-navigator`,
    background: "color-mix(in srgb, var(--portal-navy) 90%, transparent)",
    text: "var(--portal-on-navy)",
  },
  canvas: {
    family: "canvas",
    className: `${PANE_BASE} projects-pane projects-pane-canvas`,
    background: "var(--portal-panel-bg)",
    text: "var(--portal-text)",
  },
  inspector: {
    family: "ivory",
    className: `${PANE_BASE} projects-pane projects-pane-inspector`,
    background: "color-mix(in srgb, var(--portal-ivory) 94%, transparent)",
    text: "var(--portal-text)",
  },
} as const

export type ProjectsPrimitiveSurface = "navy" | "canvas" | "ivory"

/**
 * Reusable row/input/button/avatar class builders. Every builder returns a
 * complete, token-driven class string; the workspace consumes these instead of
 * hand-rolling per-surface styles. The literal fragments live here so Tailwind
 * can statically extract them.
 */
export const PROJECTS_PRIMITIVES = {
  row({ selected = false, surface = "canvas" }: { selected?: boolean; surface?: ProjectsPrimitiveSurface } = {}): string {
    const base =
      "flex w-full items-center gap-2 rounded-[var(--portal-tab-radius)] px-2 py-2 text-left transition"
    // The selected state is surface-aware: a navy tint on the navy canvas would be
    // invisible, so the midnight surface lifts with white instead.
    const state = selected
      ? surface === "navy"
        ? "bg-white/12 ring-1 ring-inset ring-white/20"
        : "bg-[var(--portal-navy)]/[0.06] ring-1 ring-inset ring-[var(--portal-navy)]/10"
      : surface === "navy"
        ? "hover:bg-white/10"
        : "hover:bg-white/40"
    return `${base} ${state}`
  },
  input({ surface = "canvas" }: { surface?: ProjectsPrimitiveSurface } = {}): string {
    return [
      "w-full rounded-[var(--portal-tab-radius)] border px-2.5 py-1.5 text-[13px] font-light outline-none transition",
      surface === "navy"
        ? "border-white/15 bg-white/10 text-white placeholder:text-white/45"
        : "border-[var(--portal-panel-border)] bg-white/60 text-[var(--portal-navy)] focus:border-[var(--portal-gold)]",
    ].join(" ")
  },
  button({ variant = "primary" }: { variant?: "primary" | "ghost" | "danger" } = {}): string {
    const base =
      "inline-flex items-center justify-center rounded-[var(--portal-tab-radius)] px-3 py-2 text-[12px] font-medium transition disabled:opacity-50"
    if (variant === "ghost") {
      return `${base} border border-[var(--portal-panel-border)] text-[var(--portal-navy)] hover:bg-white/50`
    }
    if (variant === "danger") {
      return `${base} border border-[var(--portal-archive)]/40 text-[var(--portal-archive)] disabled:opacity-40`
    }
    return `${base} bg-[var(--portal-navy)] text-white hover:opacity-90`
  },
  avatar({ size = "md" }: { size?: "sm" | "md" | "lg" } = {}): string {
    const dim = size === "sm" ? "h-7 w-7 text-[10px]" : size === "lg" ? "h-10 w-10 text-[12px]" : "h-9 w-9 text-[11px]"
    return `flex shrink-0 items-center justify-center rounded-full bg-[var(--portal-gold)]/15 font-medium text-[var(--portal-gold-muted)] ring-1 ring-inset ring-[var(--portal-gold)]/25 ${dim}`
  },
} as const

export type ProjectsLongContentPolicy = "truncate" | "wrap" | "scroll"

/**
 * Long-content policy. Single-line labels truncate, prose wraps inside its pane,
 * and lists scroll independently rather than stretching the viewport-bounded
 * grid.
 */
export const PROJECTS_LONG_CONTENT = {
  label: { policy: "truncate", classes: "min-w-0 truncate" },
  prose: { policy: "wrap", classes: "break-words [overflow-wrap:anywhere]" },
  list: { policy: "scroll", classes: "min-h-0 overflow-y-auto" },
} as const

/** Independent scroll region for a pane body (the `min-h-0` flex chain). */
export const PROJECTS_SCROLL_CLASS = "projects-scroll min-h-0 overflow-y-auto"
