'use client'

// ---------------------------------------------------------------------------
// ENGINEERING QUEUES — the working screen (iteration 1: layout + mechanic).
//
// Four queues and a stats strip. The distinction this screen exists to make is
// OWNERSHIP: the Work Bench is the human's, everything to its right is the
// engine's. Moving a card between them IS the handoff ("I'll do it" vs "the
// engine does it") and it is the one interaction the old screens never had.
//
// RESULTS holds ATTEMPTS, not stories, because a story that reaches HOLD gets
// replayed: that is why the outcome is a badge on a row and HOLD is not a queue.
// Double-clicking a RESULTS row opens that attempt in the Flight Recorder.
//
// Static data on purpose (see fixture.ts). The seam is loadEngineeringQueues().
// ---------------------------------------------------------------------------

import { useMemo, useState } from 'react'

import type { StoryBoardCockpitData, StoryLifecycle, StoryRecord } from '@/lib/storyboard-data'
import type { StoryboardStory, StoryRun } from '@/db/storyboard'
import { ActiveQueue, RunHistory, StoryDetail } from '@/components/portal/tech/engineering-cockpit'

import { loadEngineeringQueues } from './fixture'
import type { QueueCard, QueueKey, RunOutcome } from './types'

/**
 * What the route loads for us. Tiles and the story log come from ONE structure
 * (cockpit.kpis + cockpit.panels) so they can never disagree on screen, and the
 * Work Bench comes from `storyboard_active_work` — the table that already existed
 * and already orders the day's work (`listActiveWork`).
 */
export type EngineeringQueuesPageProps = {
  cockpit: StoryBoardCockpitData
  activeWork: StoryboardStory[]
  /** The story the Bench detail pane is showing. */
  selectedStory: StoryRecord | null
  selectedIsActive: boolean
  runs: StoryRun[]
  freshness: string
}

const QUEUES: Array<{
  key: QueueKey
  label: string
  owner: 'HUMAN' | 'ENGINE'
  hint: string
  ring: string
  dot: string
}> = [
  {
    key: 'bench',
    label: 'WORK BENCH',
    owner: 'HUMAN',
    hint: 'What I am working on today',
    ring: 'border-amber-400/30',
    dot: 'bg-amber-400',
  },
  {
    key: 'ready',
    label: 'ENGINE READY',
    owner: 'ENGINE',
    hint: 'Marked to hand to Forge',
    ring: 'border-sky-400/30',
    dot: 'bg-sky-400',
  },
  {
    key: 'running',
    label: 'RUNNING',
    owner: 'ENGINE',
    hint: 'The batch in the machine now',
    ring: 'border-emerald-400/30',
    dot: 'bg-emerald-400',
  },
  {
    key: 'results',
    label: 'RESULTS',
    owner: 'ENGINE',
    hint: 'Finished attempts — double-click for the Flight Recorder',
    ring: 'border-slate-400/20',
    dot: 'bg-slate-400',
  },
]

const OUTCOME: Record<RunOutcome, string> = {
  DONE: 'bg-emerald-500/15 text-emerald-300 ring-emerald-400/30',
  HOLD: 'bg-amber-500/15 text-amber-300 ring-amber-400/30',
  ERROR: 'bg-rose-500/15 text-rose-300 ring-rose-400/30',
  INTERRUPTED: 'bg-slate-500/15 text-slate-300 ring-slate-400/30',
}

export function EngineeringQueuesPage({
  cockpit,
  activeWork,
  selectedStory,
  selectedIsActive,
  runs,
  freshness,
}: EngineeringQueuesPageProps) {
  const model = useMemo(() => loadEngineeringQueues(), [])
  const [cards, setCards] = useState<QueueCard[]>(model.cards)
  const [selected, setSelected] = useState<string | null>(null)
  // The story log is the thing the captain is looking FOR, so it starts open.
  const [showLifecycle, setShowLifecycle] = useState(true)

  /** The ownership switch: one card, one queue. Never two. */
  function move(id: string, to: QueueKey) {
    setCards((prev) => prev.map((c) => (c.id === id ? { ...c, queue: to } : c)))
    setSelected(id)
  }

  function openRecorder(card: QueueCard) {
    if (!card.instanceId) return
    window.open(`/portal/tech/flight-recorder/${card.instanceId}`, '_blank', 'noopener')
  }

  const byQueue = (key: QueueKey) => cards.filter((c) => c.queue === key)
  const stats = model.stats

  // Tiles are DERIVED from the cockpit projection, not typed by hand: the same
  // numbers that fill the story log below fill these, so the top strip and the
  // bottom boxes cannot drift apart on screen.
  const tiles = [
    { label: 'TOTAL STORIES', value: String(cockpit.kpis.total), caption: 'All canonical rows' },
    { label: 'ACTIVE QUEUE', value: String(activeWork.length), caption: 'Selected today — the Work Bench' },
    { label: 'OPEN', value: String(cockpit.kpis.open), caption: 'Current work queue' },
    { label: 'BACKLOG', value: String(cockpit.kpis.backlog), caption: 'Current-version planned' },
    { label: 'CLOSED', value: String(cockpit.kpis.complete), caption: 'Finished history' },
    { label: 'COMPLETION', value: `${cockpit.kpis.completionPercent.toFixed(1)}%`, caption: 'Net-net' },
  ]

  return (
    <div className="min-h-screen bg-[#0b1220] px-5 py-6 text-slate-200">
      <header className="mb-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="text-[11px] font-semibold tracking-[0.22em] text-[#c6a15b]">{model.eyebrow}</p>
            <h1 className="mt-1 font-serif text-2xl font-semibold text-white">{model.title}</h1>
            <p className="mt-1 max-w-3xl text-sm font-light text-slate-400">{model.subtitle}</p>
            <p className="mt-1 text-[11px] text-slate-500">Story data as of {freshness}</p>
          </div>
          <a
            href="/portal/tech/flight-recorder"
            className="rounded-md border border-[#c6a15b]/50 bg-[#c6a15b]/15 px-3 py-1 text-[10px] font-medium uppercase tracking-[0.14em] text-[#e0c489]"
          >
            Flight Recorder
          </a>
        </div>
      </header>

      <section className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
        {tiles.map((tile) => (
          <div key={tile.label} className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
            <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">{tile.label}</p>
            <p className="mt-0.5 font-serif text-xl text-white">{tile.value}</p>
            <p className="text-[10px] text-slate-500">{tile.caption}</p>
          </div>
        ))}
      </section>

      {/* WORK BENCH — the piece the captain uses most: the queue on the LEFT, the
          selected story's full detail on the RIGHT, run history beneath it. It
          reuses ActiveQueue / StoryDetail / RunHistory from the TECH cockpit, so
          there is exactly ONE implementation of "what the heck is this story". */}
      <section className="mb-4">
        <div className="mb-2 flex flex-wrap items-baseline justify-between gap-2">
          <p className="text-[11px] font-semibold tracking-[0.16em] text-slate-300">
            BENCH — scope today&apos;s work{' '}
            <span className="font-normal text-[#c6a15b]">({activeWork.length})</span>
          </p>
          <p className="text-[10px] text-slate-400">
            my hands, not the engine&apos;s · pick one to read it
          </p>
        </div>
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
          <ActiveQueue
            activeQueue={activeWork}
            selectedId={selectedStory?.id ?? null}
            basePath="/portal/tech/queues"
          />
          <div className="space-y-3">
            {selectedStory ? (
              <StoryDetail story={selectedStory} isActive={selectedIsActive} />
            ) : (
              <p className="rounded-lg border border-white/10 bg-white/[0.03] px-4 py-10 text-center text-xs italic text-slate-500">
                Select a story from the Work Bench to inspect it
              </p>
            )}
            <RunHistory storyId={selectedStory?.id ?? null} runs={runs} />
          </div>
        </div>
      </section>

      <StatsStrip stats={stats} />

      {/* LINE — the four queues, the captain's own names: WORK BENCH (human) is the
          leftmost because ownership is the point; ENGINE READY / RUNNING / RESULTS
          are the engine's. HOLD is a RESULT badge, not a column. */}
      <p className="mb-2 text-[11px] font-semibold tracking-[0.16em] text-slate-300">
        LINE — the four queues
      </p>
      <section className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {QUEUES.map((queue) => {
          const items = byQueue(queue.key)
          return (
            <div
              key={queue.key}
              className={`flex min-h-[420px] flex-col rounded-lg border ${queue.ring} bg-white/[0.02]`}
            >
              <div className="flex items-start justify-between gap-2 border-b border-white/10 px-3 py-2">
                <div>
                  <p className="flex items-center gap-2 text-[11px] font-semibold tracking-[0.16em] text-white">
                    <span className={`inline-block h-1.5 w-1.5 rounded-full ${queue.dot}`} />
                    {queue.label}
                  </p>
                  <p className="mt-0.5 text-[10px] font-light text-slate-500">{queue.hint}</p>
                </div>
                <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold tracking-wider text-slate-400 ring-1 ring-white/10">
                  {queue.owner}
                </span>
              </div>
              <p className="px-3 pt-2 text-[10px] uppercase tracking-[0.14em] text-slate-500">
                {items.length} {queue.key === 'results' ? 'attempts' : 'stories'}
              </p>
              <div className="flex-1 space-y-2 overflow-y-auto px-3 pb-3 pt-2">
                {items.length === 0 ? (
                  <p className="rounded border border-dashed border-white/10 px-3 py-6 text-center text-xs text-slate-500">
                    Empty
                  </p>
                ) : (
                  items.map((card) => (
                    <QueueCardView
                      key={`${card.id}-${card.attempt ?? 0}`}
                      card={card}
                      selected={selected === card.id}
                      onSelect={() => setSelected(card.id)}
                      onMove={(to) => move(card.id, to)}
                      onOpen={() => openRecorder(card)}
                    />
                  ))
                )}
              </div>
            </div>
          )
        })}
      </section>

      <StoryLog
        panels={cockpit.panels}
        open={showLifecycle}
        onToggle={() => setShowLifecycle((v) => !v)}
      />

      <p className="mt-4 text-[11px] text-slate-500">
        Tiles, Work Bench and Story Log are LIVE from PROD (as of {stats.asOf}): tiles and boxes come from
        one projection so they cannot disagree, the bench is <code className="text-slate-400">storyboard_active_work</code> in
        work order. The four queue cards in the middle are still a labelled SAMPLE — wiring them is{' '}
        <code className="text-slate-400">loadEngineeringQueues()</code>.
      </p>
    </div>
  )
}

function StatsStrip({ stats }: { stats: ReturnType<typeof loadEngineeringQueues>['stats'] }) {
  const cells: Array<{ label: string; value: string; tone?: string }> = [
    { label: 'TOTAL RUNS', value: String(stats.runs) },
    { label: '% COMPLETE', value: `${stats.pctComplete}%`, tone: 'text-emerald-300' },
    { label: '% HOLD', value: `${stats.pctHold}%`, tone: 'text-amber-300' },
    { label: '% RERUN', value: `${stats.pctRerun}%`, tone: 'text-[#c6a15b]' },
    { label: '% FAIL', value: `${stats.pctFail}%`, tone: 'text-rose-300' },
  ]
  return (
    <section className="mb-4 rounded-lg border border-[#c6a15b]/25 bg-[#c6a15b]/[0.04] px-4 py-3">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex flex-wrap items-end gap-6">
          {cells.map((c) => (
            <div key={c.label}>
              <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">{c.label}</p>
              <p className={`font-serif text-xl ${c.tone ?? 'text-white'}`}>{c.value}</p>
            </div>
          ))}
        </div>
        <div className="text-right">
          <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">WORST OFFENDER</p>
          <p className="text-xs text-slate-300">
            {stats.worstOffender.id} · {stats.worstOffender.runs} runs
          </p>
          <p className="text-[10px] text-slate-500">
            across {stats.stories} stories · cumulative as of {stats.asOf}
          </p>
        </div>
      </div>
      <p className="mt-2 text-[11px] font-light text-slate-500">
        Rerun is the honest health number: {stats.pctRerun}% of all runs are a story&apos;s second or later
        attempt, so total runs measures churn, not throughput.
      </p>
    </section>
  )
}

function QueueCardView({
  card,
  selected,
  onSelect,
  onMove,
  onOpen,
}: {
  card: QueueCard
  selected: boolean
  onSelect: () => void
  onMove: (to: QueueKey) => void
  onOpen: () => void
}) {
  const isResult = card.queue === 'results'
  return (
    <div
      onClick={onSelect}
      onDoubleClick={isResult ? onOpen : undefined}
      title={isResult ? 'Double-click for the Flight Recorder' : undefined}
      className={`cursor-pointer rounded border px-3 py-2 transition ${
        selected ? 'border-[#c6a15b]/60 bg-[#c6a15b]/[0.06]' : 'border-white/10 bg-white/[0.02] hover:border-white/20'
      }`}
    >
      <div className="flex items-start justify-between gap-2">
        <span className="font-mono text-[11px] text-slate-400">{card.id}</span>
        {card.outcome ? (
          <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold ring-1 ${OUTCOME[card.outcome]}`}>
            {card.outcome}
          </span>
        ) : null}
      </div>
      <p className="mt-1 text-xs font-light leading-snug text-slate-200">{card.title}</p>
      <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-slate-500">
        <span>{card.workstream}</span>
        <span>·</span>
        <span>{card.priority}</span>
        <span>·</span>
        <span className="tabular-nums">{Math.round(card.completion)}%</span>
        {isResult && card.attempt ? (
          <>
            <span>·</span>
            <span>attempt {card.attempt}</span>
            {card.endedOn ? (
              <>
                <span>·</span>
                <span>ended on {card.endedOn}</span>
              </>
            ) : null}
          </>
        ) : null}
      </div>
      <div className="mt-2 flex flex-wrap gap-1">
        {QUEUES.filter((q) => q.key !== card.queue).map((q) => (
          <button
            key={q.key}
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              onMove(q.key)
            }}
            className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] text-slate-400 hover:border-[#c6a15b]/40 hover:text-[#c6a15b]"
          >
            → {q.label === 'WORK BENCH' ? 'Bench' : q.label === 'ENGINE READY' ? 'Ready' : q.label === 'RUNNING' ? 'Run' : 'Results'}
          </button>
        ))}
      </div>
    </div>
  )
}

const LIFECYCLE_LABELS: Array<{ key: StoryLifecycle; label: string }> = [
  { key: 'open', label: 'CURRENT WORK QUEUE' },
  { key: 'backlog', label: 'CURRENT-VERSION WAITING' },
  { key: 'closed', label: 'FINISHED HISTORY' },
  { key: 'next-version', label: 'INTENTIONALLY FUTURE' },
]

/**
 * The story log — the captain's four boxes, from the SAME projection that fills
 * the tiles above (cockpit.panels), grouped by workstream. Real rows, real order.
 */
function StoryLog({
  panels,
  open,
  onToggle,
}: {
  panels: StoryBoardCockpitData['panels']
  open: boolean
  onToggle: () => void
}) {
  return (
    <section className="mt-4 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-3">
      <button
        type="button"
        onClick={onToggle}
        className="flex w-full items-center justify-between gap-4 text-left"
      >
        <span className="text-[11px] font-semibold tracking-[0.16em] text-slate-300">
          UNIVERSE — what exists · STORY LOG {open ? '(hide)' : '(show)'}
        </span>
        <span className="text-[11px] text-slate-500">{open ? '▲' : '▼'}</span>
      </button>
      <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {LIFECYCLE_LABELS.map(({ key, label }) => {
          const panel = panels[key]
          return (
            <div key={key} className="flex flex-col rounded border border-white/10">
              <div className="border-b border-white/10 px-3 py-2">
                <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-300">{label}</p>
                <p className="text-[10px] text-slate-500">{panel?.count ?? 0} stories</p>
              </div>
              {open ? (
                <div className="max-h-[540px] overflow-y-auto px-3 py-2">
                  {(panel?.groups ?? []).map((g) => (
                    <div key={g.group} className="mb-2 last:mb-0">
                      <p className="text-[10px] font-semibold tracking-[0.12em] text-[#c6a15b]/80">
                        {g.group}
                      </p>
                      <ul className="mt-1 space-y-1">
                        {g.stories.map((s) => (
                          <li key={s.id} className="rounded border border-white/5 bg-white/[0.02] px-2 py-1">
                            <div className="flex items-center justify-between gap-2">
                              <span className="truncate font-mono text-[10px] text-slate-400">{s.id}</span>
                              <span className="shrink-0 text-[10px] text-slate-500">{s.status}</span>
                            </div>
                            <p className="truncate text-[11px] font-light text-slate-300" title={s.title}>
                              {s.title}
                            </p>
                            <p className="text-[10px] text-slate-500">
                              {s.priority} · {Math.round(s.completion)}%
                            </p>
                          </li>
                        ))}
                      </ul>
                    </div>
                  ))}
                  {(panel?.count ?? 0) === 0 ? (
                    <p className="text-[11px] text-slate-500">Empty</p>
                  ) : null}
                </div>
              ) : null}
            </div>
          )
        })}
      </div>
    </section>
  )
}

