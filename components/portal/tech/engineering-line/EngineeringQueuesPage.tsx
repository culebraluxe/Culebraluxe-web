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

import { loadEngineeringQueues } from './fixture'
import type { QueueCard, QueueKey, RunOutcome } from './types'

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

export function EngineeringQueuesPage() {
  const model = useMemo(() => loadEngineeringQueues(), [])
  const [cards, setCards] = useState<QueueCard[]>(model.cards)
  const [selected, setSelected] = useState<string | null>(null)
  const [showLifecycle, setShowLifecycle] = useState(false)

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

  return (
    <div className="min-h-screen bg-[#0b1220] px-5 py-6 text-slate-200">
      <header className="mb-5">
        <p className="text-[11px] font-semibold tracking-[0.22em] text-[#c6a15b]">{model.eyebrow}</p>
        <h1 className="mt-1 font-serif text-2xl font-semibold text-white">{model.title}</h1>
        <p className="mt-1 max-w-3xl text-sm font-light text-slate-400">{model.subtitle}</p>
        <p className="mt-1 text-[11px] text-slate-500">Story data as of {model.asOf}</p>
      </header>

      <section className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
        {model.tiles.map((tile) => (
          <div key={tile.label} className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
            <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">{tile.label}</p>
            <p className="mt-0.5 font-serif text-xl text-white">{tile.value}</p>
            <p className="text-[10px] text-slate-500">{tile.caption}</p>
          </div>
        ))}
      </section>

      <StatsStrip stats={stats} />

      <section className="grid grid-cols-1 gap-3 lg:grid-cols-2 xl:grid-cols-4">
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

      <LifecycleBand
        buckets={model.lifecycle}
        open={showLifecycle}
        onToggle={() => setShowLifecycle((v) => !v)}
      />

      <p className="mt-4 text-[11px] text-slate-500">
        Layout pass — data is static (fixture) and every number shown above was read from PROD on{' '}
        {stats.asOf}. The seam is <code className="text-slate-400">loadEngineeringQueues()</code>.
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

function LifecycleBand({
  buckets,
  open,
  onToggle,
}: {
  buckets: ReturnType<typeof loadEngineeringQueues>['lifecycle']
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
          FULL LIFECYCLE — {open ? 'hide' : 'show'} the canonical buckets
        </span>
        <span className="text-[11px] text-slate-500">{open ? '▲' : '▼'}</span>
      </button>
      <div className="mt-3 grid grid-cols-2 gap-3 xl:grid-cols-4">
        {buckets.map((b) => (
          <div key={b.key} className="rounded border border-white/10 px-3 py-2">
            <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">{b.label}</p>
            <p className="mt-0.5 flex items-baseline gap-2">
              <span className="font-serif text-lg text-white">{b.count}</span>
              <span className="text-[10px] text-slate-500">{b.caption}</span>
            </p>
          </div>
        ))}
      </div>
      {open ? (
        <div className="mt-3 grid grid-cols-1 gap-3 xl:grid-cols-4">
          {buckets.map((b) => (
            <div key={`${b.key}-sample`} className="rounded border border-white/10 px-3 py-2">
              <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">{b.label}</p>
              <ul className="mt-1.5 space-y-1.5">
                {b.sample.map((s) => (
                  <li key={s.id} className="text-[11px] leading-snug">
                    <span className="font-mono text-slate-400">{s.id}</span>{' '}
                    <span className="font-light text-slate-300">{s.title}</span>
                    <span className="ml-1 text-[10px] text-slate-500">{s.status}</span>
                  </li>
                ))}
              </ul>
              <p className="mt-1.5 text-[10px] text-slate-500">
                {b.count - b.sample.length > 0 ? `+ ${b.count - b.sample.length} more` : ''}
              </p>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  )
}

