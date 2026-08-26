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
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
      <div className="flex items-baseline justify-between gap-3 border-b border-[var(--portal-border)] px-5 py-4">
        <div>
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
            Explicitly selected today
          </div>
          <h2 className="mt-0.5 font-serif text-xl font-light leading-none text-[var(--portal-navy)]">
            Active Work Queue
          </h2>
        </div>
        <span className="rounded-full bg-[var(--portal-gold-pale)] px-3 py-1 font-serif text-sm font-light tabular-nums text-[var(--portal-gold-muted)]">
          {activeQueue.length}
        </span>
      </div>

      <div className="max-h-[26rem] overflow-y-auto p-3">
        {activeQueue.length === 0 ? (
          <p className="px-3 py-10 text-center text-xs font-light italic text-black/35">
            No active work selected. Use “Add to Active” on a story.
          </p>
        ) : (
          <div className="space-y-2">
            {activeQueue.map((story) => {
              const active = story.id === selectedId
              return (
                <div
                  key={story.id}
                  className={`rounded-md border p-3 transition ${
                    active
                      ? "border-[var(--portal-gold)]/70 bg-[var(--portal-gold-pale)] shadow-[0_0_0_1px_rgba(198,161,91,0.35)]"
                      : "border-[var(--portal-border)] bg-white/60 hover:border-[var(--portal-gold)]/40"
                  }`}
                >
                  <Link
                    href={`/portal/tech?story=${encodeURIComponent(story.id)}`}
                    className="block"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="font-mono text-[11px] text-[var(--portal-navy)]">
                          {story.id}
                        </div>
                        <div className="mt-0.5 line-clamp-2 text-sm font-light leading-5 text-black/70">
                          {story.title}
                        </div>
                      </div>
                      <span
                        className={`inline-block shrink-0 whitespace-nowrap rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${statusPillClasses(
                          story.status,
                        )}`}
                      >
                        {story.status}
                      </span>
                    </div>
                    <div className="mt-2 flex items-center justify-between gap-3">
                      <span className="truncate text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                        {storyDomainName(storyDomainOf(story) as never) ?? storyDomainOf(story)} ·{" "}
                        {workstreamName(story.workstream)}
                      </span>
                      <span className="text-[10px] font-light tabular-nums text-black/45">
                        {Math.round(story.completion)}%
                      </span>
                    </div>
                  </Link>
                  <form action={setActiveWorkAction} className="mt-2">
                    <input type="hidden" name="storyId" value={story.id} />
                    <input type="hidden" name="active" value="false" />
                    <button
                      type="submit"
                      className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40 transition hover:text-[var(--portal-archive)]"
                    >
                      Remove from Active
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
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
      <div className="border-b border-[var(--portal-border)] px-5 py-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="font-mono text-[11px] uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
              Current Story / Architecture
            </div>
            <h2 className="mt-1 font-serif text-xl font-light leading-tight text-[var(--portal-navy)]">
              {story.id} — {story.title}
            </h2>
          </div>
          <CopyButton text={formatStoryPacket(story)} label="Copy Story" />
        </div>
        <dl className="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-xs sm:grid-cols-4">
          <div>
            <dt className="uppercase tracking-[0.12em] text-black/35">Status</dt>
            <dd className="mt-0.5">
              <span className={`inline-block whitespace-nowrap rounded-full px-2.5 py-0.5 text-[10px] font-light uppercase tracking-[0.14em] ${statusPillClasses(story.status)}`}>
                {story.status}
              </span>
            </dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-black/35">Priority</dt>
            <dd className="mt-0.5 font-light text-black/70">{story.priority}</dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-black/35">Completion</dt>
            <dd className="mt-0.5 font-light text-black/70">{Math.round(story.completion)}%</dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-black/35">Workstream</dt>
            <dd className="mt-0.5 font-light text-black/70">{workstreamName(story.workstream)}</dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.12em] text-black/35">Domain</dt>
            <dd className="mt-0.5 font-light text-black/70">{domainLabel}</dd>
          </div>
          <div className="col-span-2">
            <dt className="uppercase tracking-[0.12em] text-black/35">Operating Surface</dt>
            <dd className="mt-0.5 font-light text-black/70">{story.operatingSurface ?? 'Unclassified'}</dd>
          </div>
        </dl>
        <form action={setActiveWorkAction} className="mt-3">
          <input type="hidden" name="storyId" value={story.id} />
          <input type="hidden" name="active" value={story.isActiveWork ? 'false' : 'true'} />
          <button
            type="submit"
            className={`inline-flex min-h-9 items-center rounded-md border px-3 text-[11px] font-light uppercase tracking-[0.14em] transition ${
              story.isActiveWork
                ? 'border-[var(--portal-archive)]/40 text-[var(--portal-archive)] hover:border-[var(--portal-archive)]'
                : 'border-[var(--portal-gold)]/50 text-[var(--portal-gold-muted)] hover:border-[var(--portal-gold)]'
            }`}
          >
            {story.isActiveWork ? 'Remove from Active' : 'Add to Active'}
          </button>
        </form>
      </div>

      <div className="max-h-[26rem] space-y-4 overflow-y-auto p-5">
        {spec.map((s) => (
          <details key={s.key} open={s.open}>
            <summary className="cursor-pointer list-none text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--portal-navy-soft)]">
              {s.key}
            </summary>
            {s.value ? (
              <p className="mt-2 whitespace-pre-wrap text-sm font-light leading-6 text-black/70">
                {s.value}
              </p>
            ) : (
              <p className="mt-2 text-xs font-light italic text-black/35">Not specified.</p>
            )}
          </details>
        ))}
      </div>
    </section>
  )
}

function RunHistory({ storyId, runs }: { storyId: string | null; runs: StoryRun[] }) {
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
      <div className="flex items-baseline justify-between gap-3 border-b border-[var(--portal-border)] px-5 py-4">
        <div>
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
            Child execution / evidence
          </div>
          <h2 className="mt-0.5 font-serif text-xl font-light leading-none text-[var(--portal-navy)]">
            Engineering / Run History
          </h2>
        </div>
        <span className="rounded-full border border-[var(--portal-border)] px-3 py-1 font-serif text-sm font-light tabular-nums text-black/50">
          {runs.length}
        </span>
      </div>

      {!storyId ? (
        <p className="px-5 py-10 text-center text-xs font-light italic text-black/35">
          Select a story to view its engineering / run history.
        </p>
      ) : runs.length === 0 ? (
        <p className="px-5 py-10 text-center text-xs font-light italic text-black/35">
          No runs recorded for {storyId} yet.
        </p>
      ) : (
        <div className="max-h-[26rem] space-y-2 overflow-y-auto p-3">
          {runs.map((run, index) => (
            <details key={run.id} className="rounded-md border border-[var(--portal-border)] bg-white/60">
              <summary className="cursor-pointer list-none px-3 py-2">
                <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
                  <span className="w-8 shrink-0 font-serif font-light tabular-nums text-black/40">
                    #{runs.length - index}
                  </span>
                  <span className="min-w-14 shrink-0 font-mono text-[10px] uppercase text-[var(--portal-navy)]">
                    {run.runType ?? '—'}
                  </span>
                  <span className="min-w-16 shrink-0 font-light text-black/60">
                    {run.agentRuntime ?? '—'}
                  </span>
                  <span className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-light uppercase tracking-[0.12em] ${statusPillClasses(run.resultStatus ?? 'Hold')}`}>
                    {run.resultStatus ?? '—'}
                  </span>
                  <span className="shrink-0 tabular-nums text-black/50">{run.completion ?? 0}%</span>
                  <span className="min-w-16 truncate font-mono text-[10px] text-black/45">{run.commitHash ?? '—'}</span>
                  <span className="shrink-0 truncate text-[10px] text-black/40">{run.executionEnvironment ?? '—'}</span>
                  <span className="shrink-0 text-[10px] text-black/40">{run.startedAt ? run.startedAt.slice(0, 10) : '—'}</span>
                </div>
              </summary>
              <div className="border-t border-[var(--portal-border)] px-3 py-3">
                <div className="mb-2 flex items-center justify-between gap-3">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
                    Frozen execution snapshot
                  </p>
                  <CopyButton text={formatRunPacket(storyId, run)} label="Copy Run" />
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-black/60">
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Result</dt><dd>{run.resultStatus ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Completion</dt><dd>{run.completion ?? 0}%</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Started</dt><dd>{run.startedAt || '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Ended</dt><dd>{run.endedAt ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Environment</dt><dd>{run.executionEnvironment ?? '—'}</dd></div>
                  <div><dt className="uppercase tracking-[0.12em] text-black/35">Commit</dt><dd className="truncate font-mono text-[10px]">{run.commitHash ?? '—'}</dd></div>
                </dl>
                {run.testsSummary && (
                  <div className="mt-2">
                    <div className="text-[10px] uppercase tracking-[0.12em] text-black/35">Tests</div>
                    <p className="mt-0.5 whitespace-pre-wrap text-xs font-light text-black/65">{run.testsSummary}</p>
                  </div>
                )}
                {run.notes && (
                  <div className="mt-2">
                    <div className="text-[10px] uppercase tracking-[0.12em] text-black/35">Notes</div>
                    <p className="mt-0.5 whitespace-pre-wrap text-xs font-light text-black/65">{run.notes}</p>
                  </div>
                )}
                <div className="mt-2 space-y-1 border-t border-black/5 pt-2 text-xs text-black/55">
                  <div><span className="uppercase tracking-[0.12em] text-black/35">Frozen Goal</span><p className="mt-0.5 whitespace-pre-wrap">{run.goalSnapshot ?? '—'}</p></div>
                  <div><span className="uppercase tracking-[0.12em] text-black/35">Frozen Architecture Brief</span><p className="mt-0.5 whitespace-pre-wrap">{run.architectBriefSnapshot ?? '—'}</p></div>
                  <div><span className="uppercase tracking-[0.12em] text-black/35">Frozen Acceptance Criteria</span><p className="mt-0.5 whitespace-pre-wrap">{run.acceptanceCriteriaSnapshot ?? '—'}</p></div>
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
    <div>
      <header className="mb-6">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
              TECH / Engineering
            </p>
            <h1 className="mt-2 font-serif text-4xl font-light leading-[1.1] text-[var(--portal-navy)]">
              Engineering Cockpit
            </h1>
          </div>
          <span className="rounded-full border border-[var(--portal-border)] px-3 py-1 text-[11px] font-light text-black/45">
            Story data as of {freshness.slice(0, 16).replace('T', ' ')} UTC
          </span>
        </div>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Program control room: current intent, canonical story architecture, and
          Forge/engineering execution evidence on one screen.
        </p>
      </header>

      {/* KPI strip (canonical + active-queue counts). */}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <KpiCard label="Total stories" value={String(kpis.total)} note="All canonical board rows" />
        <KpiCard label="Active queue" value={String(activeQueue.length)} accent note="Explicitly selected today" />
        <KpiCard label="Open" value={String(kpis.open)} note="Current work queue" />
        <KpiCard label="Backlog" value={String(kpis.backlog)} note="Current-version planned" />
        <KpiCard label="Closed" value={String(kpis.complete)} note="Finished history" />
        <KpiCard label="Completion" value={`${kpis.completionPercent.toFixed(1)}%`} note="Net-net of the five domains" />
      </div>

      {/* Active Engineering Workspace. */}
      <div className="mt-6 grid gap-5 lg:grid-cols-3">
        <ActiveQueue activeQueue={activeQueue} selectedId={selectedStory?.id ?? null} />
        <div className="flex flex-col gap-5 lg:col-span-2">
          {selectedStory ? (
            <StoryDetail story={selectedStory} />
          ) : (
            <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel px-5 py-12 text-center">
              <p className="text-sm font-light italic text-black/40">
                Select a story from the Active Work Queue (or Open/Backlog) to see its canonical
                specification and run history.
              </p>
            </section>
          )}
          <RunHistory storyId={selectedStory?.id ?? null} runs={runs} />
        </div>
      </div>

      {/* Preserved PORTAL-12 four-lifecycle cockpit. */}
      <h2 className="mt-8 font-serif text-2xl font-light text-[var(--portal-navy)]">
        Full Lifecycle
      </h2>
      <StoryBoardLifecycleGrid panels={cockpit.panels} />
    </div>
  )
}
