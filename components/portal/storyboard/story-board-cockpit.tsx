import Link from "next/link"

import {
  LIFECYCLE_META,
  type CockpitPanel,
  type StoryBoardCockpitData,
  type StoryRecord,
} from "@/lib/storyboard-data"
import { statusPillClasses } from "@/components/portal/storyboard/story-detail-sections"

// ---------------------------------------------------------------------------
// PORTAL-12 — Story Board Operating Cockpit.
//
// Server-rendered glass operating cockpit: a top KPI strip + a 2×2 lifecycle
// layout (OPEN / BACKLOG / CLOSED / NEXT VERSION). Every value comes from the
// canonical Story Board projection (buildStoryBoardCockpit); no second state
// system and no status reinterpretation. CLOSED is intentionally subdued so
// history never dominates the current work queue. Each panel scrolls within its
// own bounded region.
// ---------------------------------------------------------------------------

export function KpiCard({
  label,
  value,
  note,
  accent = false,
  tone = "light",
}: {
  label: string
  value: string
  note?: string
  accent?: boolean
  tone?: "light" | "dark"
}) {
  const dark = tone === "dark"
  return (
    <div
      className={`relative overflow-hidden rounded-[var(--portal-panel-radius)] border p-3.5 ${
        dark
          ? "portal-glass-panel-feature"
          : accent
            ? "border-[var(--portal-gold)]/40 [background:var(--portal-feature-gradient)] text-white shadow-[var(--portal-feature-shadow)]"
            : "border-[var(--portal-panel-border)] bg-white text-[var(--portal-navy)] shadow-[var(--portal-panel-shadow)]"
      }`}
    >
      <div
        className={`text-[9px] font-light uppercase tracking-[0.18em] ${
          dark || accent ? "text-[var(--portal-feature-eyebrow)]" : "text-[var(--portal-blue-gray)]"
        }`}
      >
        {label}
      </div>
      <div
        className={`mt-1 font-serif text-2xl font-light leading-none tabular-nums ${
          dark || accent ? "text-white" : "text-[var(--portal-navy)]"
        }`}
      >
        {value}
      </div>
      {note && (
        <div
          className={`mt-1 text-[10px] font-light leading-4 ${
            dark || accent ? "text-white/55" : "text-black/40"
          }`}
        >
          {note}
        </div>
      )}
    </div>
  )
}

function StoryMiniCard({
  story,
  subdued,
  tone = "light",
}: {
  story: StoryRecord
  subdued?: boolean
  tone?: "light" | "dark"
}) {
  const dark = tone === "dark"
  return (
    <Link
      href={`/portal/storyboard/${encodeURIComponent(story.id)}`}
      className={`group block rounded-md border px-2.5 py-1.5 transition ${
        dark
          ? subdued
            ? "border-white/5 bg-white/[0.02] hover:border-[var(--portal-gold)]/30"
            : "border-white/10 bg-white/[0.03] hover:border-[var(--portal-gold)]/40"
          : subdued
            ? "border-black/5 bg-black/[0.02]"
            : "border-[var(--portal-border)] bg-white/60 hover:border-[var(--portal-gold)]/50"
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className={`font-mono text-[11px] ${dark ? "text-[var(--portal-on-navy)]" : subdued ? "text-black/35" : "text-[var(--portal-navy)]"}`}>
            {story.id}
          </div>
          <div className={`mt-0.5 line-clamp-2 text-sm font-light leading-5 ${dark ? "text-white/70" : subdued ? "text-black/45" : "text-black/70"}`}>
            {story.title}
          </div>
        </div>
        <span
          className={`inline-block shrink-0 whitespace-nowrap rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
            dark
              ? "border border-white/10 text-[var(--portal-on-navy)]"
              : subdued
                ? "border border-black/10 text-black/35"
                : statusPillClasses(story.status)
          }`}
        >
          {story.status}
        </span>
      </div>
      <div className="mt-2 flex items-center justify-between gap-3">
        <span className={`text-[10px] font-light uppercase tracking-[0.12em] ${dark ? "text-white/40" : subdued ? "text-black/30" : "text-black/40"}`}>
          {story.priority}
        </span>
        <div className="flex min-w-24 items-center gap-2">
          <div className={`h-1 flex-1 overflow-hidden rounded-full ${dark ? "bg-white/15" : "bg-black/10"}`}>
            <div
              className={`h-full rounded-full ${dark ? "bg-[var(--portal-gold)]/80" : subdued ? "bg-black/25" : "bg-[var(--portal-navy)]"}`}
              style={{ width: `${Math.max(0, Math.min(100, story.completion))}%` }}
            />
          </div>
          <span className={`text-[10px] font-light tabular-nums ${dark ? "text-white/45" : subdued ? "text-black/30" : "text-black/50"}`}>
            {Math.round(story.completion)}%
          </span>
        </div>
      </div>
    </Link>
  )
}

function LifecyclePanel({
  panel,
  subdued,
  attention,
  tone = "light",
}: {
  panel: CockpitPanel
  subdued?: boolean
  attention?: boolean
  tone?: "light" | "dark"
}) {
  const meta = LIFECYCLE_META.find((m) => m.bucket === panel.bucket)!
  const dark = tone === "dark"
  return (
    <section
      className={`overflow-hidden rounded-[var(--portal-panel-radius)] ${
        dark ? "portal-glass-panel-feature" : attention ? "portal-glass-panel-attention" : "portal-glass-panel"
      } ${subdued ? (dark ? "opacity-80" : "opacity-90") : ""}`}
    >
      <div className={`flex items-baseline justify-between gap-3 border-b px-4 py-3 ${dark ? "border-white/10" : "border-[var(--portal-border)]"}`}>
        <div>
          <div className={`text-[9px] font-light uppercase tracking-[0.18em] ${dark ? "text-[var(--portal-on-navy)]/60" : "text-[var(--portal-blue-gray)]"}`}>
            {meta.eyebrow}
          </div>
          <h2 className={`mt-0.5 font-serif text-lg font-semibold leading-none ${dark ? "text-white" : subdued ? "text-black/55" : "text-[var(--portal-navy)]"}`}>
            {meta.title}
          </h2>
        </div>
        <span
          className={`rounded-full px-2.5 py-0.5 font-serif text-sm font-light tabular-nums ${
            dark
              ? "border border-white/10 text-[var(--portal-on-navy)]"
              : attention
                ? "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]"
                : "border border-[var(--portal-border)] text-black/50"
          }`}
        >
          {panel.count}
        </span>
      </div>

      <div className="max-h-72 overflow-y-auto p-2.5">
        {panel.groups.length === 0 ? (
          <p className={`px-3 py-8 text-center text-xs font-light italic ${dark ? "text-[var(--portal-on-navy)]/45" : "text-black/35"}`}>
            No {meta.title.toLowerCase()} stories right now.
          </p>
        ) : (
          <div className="space-y-3">
            {panel.groups.map((group) => (
              <div key={group.group}>
                <div className={`mb-1.5 px-1 text-[9px] font-semibold uppercase tracking-[0.18em] ${dark ? "text-[var(--portal-feature-eyebrow)]/70" : subdued ? "text-black/30" : "text-[var(--portal-navy-soft)]"}`}>
                  {group.group}
                </div>
                <div className="space-y-1.5">
                  {group.stories.map((story) => (
                    <StoryMiniCard key={story.id} story={story} subdued={subdued} tone={tone} />
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}

export function StoryBoardKpiStrip({ kpis }: { kpis: StoryBoardCockpitData['kpis'] }) {
  return (
    <div className="mt-6 grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <KpiCard label="Total stories" value={String(kpis.total)} note="All canonical board rows" />
      <KpiCard label="Open" value={String(kpis.open)} accent note="Current work queue" />
      <KpiCard label="Backlog" value={String(kpis.backlog)} note="Current-version planned" />
      <KpiCard label="Blocked / Hold" value={String(kpis.blockedHold)} accent note="Attention required" />
      <KpiCard label="Complete" value={String(kpis.complete)} note="Finished history" />
      <KpiCard
        label="Completion"
        value={`${kpis.completionPercent.toFixed(1)}%`}
        note="Net-net of the five domains"
      />
    </div>
  )
}

export function StoryBoardLifecycleGrid({
  panels,
  tone = "light",
}: {
  panels: StoryBoardCockpitData['panels']
  tone?: "light" | "dark"
}) {
  return (
    <div className="mt-4 grid gap-4 lg:grid-cols-2">
      <LifecyclePanel panel={panels.open} attention tone={tone} />
      <LifecyclePanel panel={panels.backlog} tone={tone} />
      <LifecyclePanel panel={panels.closed} subdued tone={tone} />
      <LifecyclePanel panel={panels['next-version']} tone={tone} />
    </div>
  )
}

export function StoryBoardCockpit({ cockpit }: { cockpit: StoryBoardCockpitData }) {
  return (
    <div>
      <StoryBoardKpiStrip kpis={cockpit.kpis} />
      <StoryBoardLifecycleGrid panels={cockpit.panels} />
    </div>
  )
}
