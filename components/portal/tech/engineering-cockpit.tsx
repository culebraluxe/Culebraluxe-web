import Link from "next/link"

import {
  KpiCard,
  StoryBoardLifecycleGrid,
} from "@/components/portal/storyboard/story-board-cockpit"
import { CopyButton } from "@/components/portal/tech/copy-button"
import { statusPillClasses } from "@/components/portal/storyboard/story-detail-sections"
import {
  formatRunPacket,
  formatStoryPacket,
  storyDomainName,
  storyDomainOf,
  workstreamName,
  type StoryBoardCockpitData,
  type StoryRecord,
} from "@/lib/storyboard-data"
import type { StoryRun } from "@/db/storyboard"
import { setActiveWorkAction } from "@/app/portal/tech/actions"

// ---------------------------------------------------------------------------
// PORTAL-13 — TECH / Engineering Cockpit.
//
// The TECH landing operating page: KPI strip + Active Engineering Workspace
// (Active Queue | Current Story/Architecture + Run History) + the preserved
// PORTAL-12 OPEN/BACKLOG/CLOSED/NEXT VERSION lifecycle cockpit.
//
// Active Queue is an EXPLICIT selection (isActiveWork + active_work_order). It
// never changes story status and never launches work. Current story truth comes
// from storyboard_story; execution history + frozen spec snapshots come from
// storyboard_story_run.
// ---------------------------------------------------------------------------

function ActiveQueue({
  activeQueue,
  selectedId,
}: {
  activeQueue: StoryRecord[]
  selectedId: string | null
}) {
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel-feature">
      <div className="flex items-baseline justify-between gap-3 border-b border-white/10 px-4 py-3">
        <div>
          <div className="text-[9px] font-light uppercase tracking-[0.18em] text-[var(--portal-on-navy)]/60">
            Explicitly selected today
          </div>
          <h2 className="mt-0.5 font-serif text-lg font-semibold leading-none text-white">
            Active Work Queue
          </h2>
        </div>
        <span className="rounded-full border border-white/10 px-2.5 py-0.5 font-serif text-sm font-light tabular-nums text-[var(--portal-gold-soft)]">
          {activeQueue.length}
        </span>
      </div>

      <div className="max-h-72 overflow-y-auto p-2">
        {activeQueue.length === 0 ? (
          <p className="px-3 py-10 text-center text-xs font-light italic text-[var(--portal-on-navy)]/50">
            No active work selected. Use “Add to Active” on a story.
          </p>
        ) : (
          <div className="space-y-1">
            {activeQueue.map((story) => {
              const active = story.id === selectedId
              return (
                <div
                  key={story.id}
                  className={`flex items-center gap-2 rounded-md border px-2.5 py-1.5 transition ${
                    active
                      ? "border-[var(--portal-gold)]/60 bg-[var(--portal-gold-pale)] shadow-[0_0_0_1px_rgba(198,161,91,0.25)]"
                      : "border-white/10 bg-white/[0.03] hover:border-[var(--portal-gold)]/30"
                  }`}
                >
                  <span
                    className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                      active ? "bg-[var(--portal-gold)]" : "bg-white/25"
                    }`}
                  />
                  <Link
                    href={`/portal/tech?story=${encodeURIComponent(story.id)}`}
                    className="flex min-w-0 flex-1 items-center gap-2"
                  >
                    <span className="w-16 shrink-0 truncate font-mono text-[11px] text-[var(--portal-on-navy)]">
                      {story.id}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[12px] font-light text-white/75">
                      {story.title}
                    </span>
                    <span className="hidden shrink-0 text-[9px] font-light uppercase tracking-[0.1em] text-white/40 sm:block">
                      {workstreamName(story.workstream)}
                    </span>
                    <span className="shrink-0 tabular-nums text-[10px] text-white/45">
                      {Math.round(story.completion)}%
                    </span>
                  </Link>
                  <form action={setActiveWorkAction}>
                    <input type="hidden" name="storyId" value={story.id} />
                    <input type="hidden" name="active" value="false" />
                    <button
                      type="submit"
                      className="shrink-0 text-[9px] font-light uppercase tracking-[0.12em] text-white/40 transition hover:text-[var(--portal-gold-soft)]"
                    >
                      Remove
                    </button>
                  </form>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </section>
  )
}

function StoryDetail({ story }: { story: StoryRecord }) {
  const domain = storyDomainOf(story)
  const domainLabel = domain === 'UNCLASSIFIED' ? 'UNCLASSIFIED' : storyDomainName(domain)
  const spec = [
    { key: 'Goal', value: story.goal, open: true },
    { key: 'Architecture Brief', value: story.architectBrief, open: true },
    { key: 'Scope', value: story.scope, open: false },
    { key: 'Preconditions', value: story.preconditions, open: false },
    { key: 'Acceptance Criteria', value: story.acceptanceCriteria, open: true },
    { key: 'Postconditions', value: story.postconditions, open: false },
    { key: 'Context / References', value: story.contextRefs, open: false },
    { key: 'Notes', value: story.notes, open: false },
  ]
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel-feature">
      <div className="border-b border-white/10 px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="font-mono text-[9px] uppercase tracking-[0.16em] text-[var(--portal-on-navy)]/60">
              Current Story / Architecture
            </div>
            <h2 className="mt-1 truncate font-serif text-lg font-semibold leading-tight text-white">
              {story.id} — {story.title}
            </h2>
          </div>
          <CopyButton text={formatStoryPacket(story)} label="Copy Story" />
        </div>
        <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-[11px] sm:grid-cols-4">
          <div>
            <dt className="uppercase tracking-[0.12em] text-white/40">Status</dt>
            <dd className="mt-0.5">
              <span className={`inline-block whitespace-nowrap rounded-full px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.12em] border border-white/15 text-[var(--portal-on-navy)]`}>
                {story.status}
              </span>
            </dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-white/40">Priority</dt>
            <dd className="mt-0.5 font-light text-white/80">{story.priority}</dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-white/40">Completion</dt>
            <dd className="mt-0.5 flex items-center gap-2">
              <span className="h-1 w-14 overflow-hidden rounded-full bg-white/15">
                <span
                  className="block h-full rounded-full bg-[var(--portal-gold)]/80"
                  style={{ width: `${Math.max(0, Math.min(100, story.completion))}%` }}
                />
              </span>
              <span className="font-light tabular-nums text-white/80">{Math.round(story.completion)}%</span>
            </dd>
          </div>
          <div className="col-span-2 sm:col-span-1">
            <dt className="uppercase tracking-[0.12em] text-white/40">Domain</dt>
            <dd className="mt-0.5 font-light text-white/80">{domainLabel}</dd>
          </div>
          <div className="col-span-2">
            <dt className="uppercase tracking-[0.12em] text-white/40">Operating Surface</dt>
            <dd className="mt-0.5 font-light text-white/80">{story.operatingSurface ?? 'Unclassified'}</dd>
          </div>
        </dl>
        <form action={setActiveWorkAction} className="mt-2.5">
          <input type="hidden" name="storyId" value={story.id} />
          <input type="hidden" name="active" value={story.isActiveWork ? 'false' : 'true'} />
          <button
            type="submit"
            className={`inline-flex min-h-8 items-center rounded-md border px-3 text-[10px] font-light uppercase tracking-[0.12em] transition ${
              story.isActiveWork
                ? 'border-[var(--portal-gold)]/40 text-[var(--portal-gold-soft)] hover:border-[var(--portal-gold)]'
                : 'border-white/15 text-[var(--portal-on-navy)] hover:border-[var(--portal-gold)]/50 hover:text-[var(--portal-gold-soft)]'
            }`}
          >
            {story.isActiveWork ? 'Remove from Active' : 'Add to Active'}
          </button>
        </form>
      </div>

      <div className="max-h-64 space-y-3 overflow-y-auto p-4">
        {spec.map((s) => (
          <details key={s.key} open={s.open}>
            <summary className="cursor-pointer list-none text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--portal-feature-eyebrow)]/80">
              {s.key}
            </summary>
            {s.value ? (
              <p className="mt-1 whitespace-pre-wrap text-[13px] font-light leading-5 text-white/75">
                {s.value}
              </p>
            ) : (
              <p className="mt-1 text-xs font-light italic text-white/40">Not specified.</p>
            )}
          </details>
        ))}
      </div>
    </section>
  )
}

function RunHistory({ storyId, runs }: { storyId: string | null; runs: StoryRun[] }) {
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel-feature">
      <div className="flex items-baseline justify-between gap-3 border-b border-white/10 px-4 py-3">
        <div>
          <div className="text-[9px] font-light uppercase tracking-[0.18em] text-[var(--portal-on-navy)]/60">
            Child execution / evidence
          </div>
          <h2 className="mt-0.5 font-serif text-lg font-semibold leading-none text-white">
            Engineering / Run History
          </h2>
        </div>
        <span className="rounded-full border border-white/10 px-2.5 py-0.5 font-serif text-sm font-light tabular-nums text-[var(--portal-on-navy)]">
          {runs.length}
        </span>
      </div>

      {!storyId ? (
        <p className="px-4 py-8 text-center text-xs font-light italic text-[var(--portal-on-navy)]/50">
          Select a story to view its engineering / run history.
        </p>
      ) : runs.length === 0 ? (
        <p className="px-4 py-8 text-center text-xs font-light italic text-[var(--portal-on-navy)]/50">
          No execution history recorded for this story.
        </p>
      ) : (
        <div className="max-h-64 space-y-1 overflow-y-auto p-2">
          {runs.map((run, index) => (
            <details key={run.id} className="rounded-md border border-white/10 bg-white/[0.03]">
              <summary className="cursor-pointer list-none px-3 py-1.5">
                <div className="flex flex-wrap items-center gap-x-4 gap-y-0.5 text-[11px]">
                  <span className="w-8 shrink-0 font-serif font-light tabular-nums text-white/40">
                    #{runs.length - index}
                  </span>
                  <span className="min-w-14 shrink-0 font-mono text-[10px] uppercase text-[var(--portal-on-navy)]">
                    {run.runType ?? '—'}
                  </span>
                  <span className="min-w-16 shrink-0 font-light text-white/70">{run.agentRuntime ?? '—'}</span>
                  <span className={`shrink-0 rounded-full border border-white/15 px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.12em] text-[var(--portal-on-navy)]`}>
                    {run.resultStatus ?? '—'}
                  </span>
                  <span className="shrink-0 tabular-nums text-white/60">{run.completion ?? 0}%</span>
                  <span className="min-w-16 truncate font-mono text-[10px] text-white/50">{run.commitHash ?? '—'}</span>
                  <span className="hidden shrink-0 truncate text-[10px] text-white/40 sm:block">{run.executionEnvironment ?? '—'}</span>
                  <span className="shrink-0 text-[10px] text-white/40">{run.startedAt ? run.startedAt.slice(0, 10) : '—'}</span>
                </div>
              </summary>
              <div className="border-t border-white/10 px-3 py-3">
                <div className="mb-2 flex items-center justify-between gap-3">
                  <p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--portal-feature-eyebrow)]/80">
                    Frozen execution snapshot
                  </p>
                  <CopyButton text={formatRunPacket(storyId, run)} label="Copy Run" />
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-white/70">
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Result</dt><dd>{run.resultStatus ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Completion</dt><dd>{run.completion ?? 0}%</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Started</dt><dd>{run.startedAt || '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Ended</dt><dd>{run.endedAt ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Environment</dt><dd>{run.executionEnvironment ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-white/40">Commit</dt><dd className="truncate font-mono text-[10px]">{run.commitHash ?? '—'}</dd></div>
                </dl>
                {run.testsSummary && (
                  <div className="mt-2">
                    <div className="text-[10px] uppercase tracking-[0.12em] text-white/40">Tests</div>
                    <p className="mt-0.5 whitespace-pre-wrap text-xs font-light text-white/70">{run.testsSummary}</p>
                  </div>
                )}
                {run.notes && (
                  <div className="mt-2">
                    <div className="text-[10px] uppercase tracking-[0.12em] text-white/40">Notes</div>
                    <p className="mt-0.5 whitespace-pre-wrap text-xs font-light text-white/70">{run.notes}</p>
                  </div>
                )}
                <div className="mt-2 space-y-1 border-t border-white/10 pt-2 text-xs text-white/60">
                  <div><span className="uppercase tracking-[0.12em] text-white/40">Frozen Goal</span><p className="mt-0.5 whitespace-pre-wrap">{run.goalSnapshot ?? '—'}</p></div>
                  <div><span className="uppercase tracking-[0.12em] text-white/40">Frozen Architecture Brief</span><p className="mt-0.5 whitespace-pre-wrap">{run.architectBriefSnapshot ?? '—'}</p></div>
                  <div><span className="uppercase tracking-[0.12em] text-white/40">Frozen Acceptance Criteria</span><p className="mt-0.5 whitespace-pre-wrap">{run.acceptanceCriteriaSnapshot ?? '—'}</p></div>
                </div>
              </div>
            </details>
          ))}
        </div>
      )}
    </section>
  )
}

export function EngineeringCockpit({
  cockpit,
  activeQueue,
  selectedStory,
  runs,
  freshness,
}: {
  cockpit: StoryBoardCockpitData
  activeQueue: StoryRecord[]
  selectedStory: StoryRecord | null
  runs: StoryRun[]
  freshness: string
}) {
  const { kpis } = cockpit
  return (
    <div
      className="px-2 py-4 sm:px-4"
      style={{
        background:
          "radial-gradient(1200px 600px at 20% -10%, rgba(41,68,95,0.55), transparent 60%), radial-gradient(900px 500px at 100% 0%, rgba(24,43,64,0.6), transparent 55%), linear-gradient(180deg, var(--portal-navy-deep-2) 0%, var(--portal-navy-deep) 100%)",
        minHeight: "100vh",
      }}
    >
      {/* Compact header. */}
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[10px] font-light uppercase tracking-[0.28em] text-[var(--portal-feature-eyebrow)]">
            TECH / Engineering
          </p>
          <h1 className="mt-1 font-serif text-2xl font-light leading-none text-white">
            Engineering Cockpit
          </h1>
          <p className="mt-2 max-w-2xl text-xs font-light leading-5 text-[var(--portal-on-navy)]/70">
            Program control room: current intent, canonical story architecture, and
            Forge/engineering execution evidence on one screen.
          </p>
        </div>
        <span className="rounded-full border border-white/10 px-3 py-1 text-[10px] font-light text-[var(--portal-on-navy)]/55">
          Story data as of {freshness.slice(0, 16).replace('T', ' ')} UTC
        </span>
      </div>

      {/* KPI strip — six compact glass cards in one row. */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
        <KpiCard tone="dark" label="Total stories" value={String(kpis.total)} note="All canonical rows" />
        <KpiCard tone="dark" accent label="Active queue" value={String(activeQueue.length)} note="Selected today" />
        <KpiCard tone="dark" label="Open" value={String(kpis.open)} note="Current work queue" />
        <KpiCard tone="dark" label="Backlog" value={String(kpis.backlog)} note="Current-version planned" />
        <KpiCard tone="dark" label="Closed" value={String(kpis.complete)} note="Finished history" />
        <KpiCard tone="dark" label="Completion" value={`${kpis.completionPercent.toFixed(1)}%`} note="Net-net" />
      </div>

      {/* Active Engineering Workspace — compact master/detail. */}
      <div className="mt-4 grid gap-4 lg:grid-cols-[42fr_58fr]">
        <ActiveQueue activeQueue={activeQueue} selectedId={selectedStory?.id ?? null} />
        <div className="flex flex-col gap-4">
          {selectedStory ? (
            <StoryDetail story={selectedStory} />
          ) : (
            <section className="portal-glass-panel-feature flex min-h-[9rem] items-center justify-center rounded-[var(--portal-panel-radius)] px-5 text-center">
              <p className="max-w-md text-sm font-light leading-6 text-[var(--portal-on-navy)]/70">
                Select a story from Active Work Queue, Open, or Backlog to inspect
                its canonical specification and execution history.
              </p>
            </section>
          )}
          <RunHistory storyId={selectedStory?.id ?? null} runs={runs} />
        </div>
      </div>

      {/* Preserved PORTAL-12 four-lifecycle cockpit (dark). */}
      <h2 className="mt-6 font-serif text-lg font-light uppercase tracking-[0.14em] text-[var(--portal-feature-eyebrow)]">
        Full Lifecycle
      </h2>
      <StoryBoardLifecycleGrid tone="dark" panels={cockpit.panels} />
    </div>
  )
}
