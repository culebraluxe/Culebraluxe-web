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
// Every lane on this board is read from PROD: the story columns from the storyboard projection,
// ENGINE QUEUED from agent_work_item, RUNNING/RESULTS from the engine ledger. There is no fixture
// left on this screen, and no interaction that only moves pixels.
// ---------------------------------------------------------------------------

import { useEffect, useMemo, useState } from 'react'
import { useRouter } from 'next/navigation'

import { moveStoryBucketAction, sendEngineBatchAction } from '@/app/portal/tech/actions'
import { storyLifecycleOf } from '@/lib/storyboard-data'
import type { StoryBucket } from '@/lib/story-moves'

import type { StoryBoardCockpitData, StoryLifecycle, StoryRecord } from '@/lib/storyboard-data'
import type { StoryboardStory, StoryRun } from '@/db/storyboard'
import {
  ActiveQueue,
  RunHistory,
  StoryDetail,
  type HistoryStory,
} from '@/components/portal/tech/engineering-cockpit'
import { StoryKanbanBoard } from '@/components/portal/tech/story-kanban-board'

import type { QueueCard, QueueKey, RunOutcome } from './types'
import type { EngineRunCard, EngineLedgerStats, EngineQueuedCard } from '@/db/forge-engine-task-execution'
import type { ForgeStoryHold } from '@/db/forge-hold'

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
  /**
   * The engine instance for the selected story, when one exists — what the Flight Recorder
   * needs to show what actually happened. Null for a story the engine has not run.
   */
  recorderInstanceId?: string | null
  /**
   * Stories that actually have run history, newest first — the signpost shown when the selected
   * story has none. Computed once by the page from the execution summaries it already loads.
   */
  historyStories?: HistoryStory[]
  /** Ids of those stories, for the "runs" badge on a bench row. */
  historyStoryIds?: string[]
  /** Total stories with run history, so a capped signpost list never claims to be the total. */
  historyTotal?: number
  /**
   * The engine's own lanes, from the engine ledger: one entry per story, newest attempt first.
   * RUNNING and RESULTS are built from this; without it they would have to fall back to fixture
   * data, which is how a card in "ENGINE DONE" ended up standing for a story the engine never ran.
   */
  engineRuns?: EngineRunCard[] | null
  /**
   * The ledger's own totals, for the stats strip. Null means the read failed and the strip says so
   * rather than showing invented numbers.
   */
  ledgerStats?: EngineLedgerStats | null
  /** Work items genuinely still open ("handed to Forge, queued, not started"). Null = read failed. */
  queuedCards?: EngineQueuedCard[] | null
  /**
   * The engine's CURRENT stop for the selected story, when it is parked.
   *
   * A HOLD is a deliberate machine stop with a reason. Until this existed, a parked story read
   * "In Progress / 100%" on the board while `forge_hold_record` held the truth.
   */
  hold?: ForgeStoryHold | null
  /**
   * Stories staged in ENGINE BATCH (status `Batched`), in display order.
   *
   * Staging is not dispatch: `Batched` writes nothing on the engine, and the batch is sent by an
   * explicit action. This is what the captain meant by "stage the next group, then kick it off".
   */
  batchStories?: Array<{
    id: string
    title: string
    status: string
    priority: string
    completion: number
  }> | null
}

const QUEUES: Array<{
  key: QueueKey
  label: string
  owner: 'HUMAN' | 'ENGINE'
  hint: string
  ring: string
  dot: string
}> = [
  // The WORK BENCH is NOT here on purpose: it is the band ABOVE this line (the
  // human lane, storyboard_active_work). Having it in both places was the name
  // collision that made the captain's head spin — the same 12 stories twice.
  // The line is the ENGINE's pipeline only, in his words: queued -> running -> done.
  {
    key: 'ready',
    label: 'ENGINE QUEUED',
    owner: 'ENGINE',
    hint: 'Handed to Forge — queued, not started',
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
  recorderInstanceId,
  runs,
  freshness,
  historyStories,
  historyStoryIds,
  historyTotal,
  engineRuns,
  ledgerStats,
  queuedCards,
  hold,
  batchStories,
}: EngineeringQueuesPageProps) {
  const router = useRouter()
  // THE ENGINE'S LANES COME FROM THE ENGINE — there is no fixture on this screen any more.
  //
  // The RUNNING / RESULTS lanes are the engine's own ("the batch the machine is executing", "finished
  // ATTEMPTS") and ENGINE QUEUED is "handed to Forge, not started". All three read real rows:
  // `agent_work_item` for work that is waiting, `forge_engine_task_execution` for work that ran. The
  // static fixture that used to feed them made the engine's lane show stories the engine had never
  // touched, and opening one produced a truthful "no instance recorded" that read as a broken screen.
  const engineCards = useMemo<QueueCard[]>(
    () => [
      // ENGINE QUEUED: real open work items, from agent_work_item. An empty lane is the truthful
      // answer when the engine has nothing waiting, and it is what this lane showed for weeks of
      // fixture cards that stood for nothing.
      ...(queuedCards ?? []).map((queued): QueueCard => ({
        id: `${queued.storyId}#queued`,
        title: queued.title,
        workstream: 'ENGINEERING',
        status: queued.state,
        priority: 'MEDIUM',
        completion: 0,
        queue: 'ready' as const,
        storyId: queued.storyId,
      })),
      ...(engineRuns ?? []).map((run): QueueCard => {
        const live =
          !run.stale &&
          (run.status === 'running' || run.status === 'claimed' || run.status === 'queued')
        const outcome: RunOutcome =
          run.status === 'completed' ? 'DONE' : run.status === 'failed' ? 'ERROR' : 'INTERRUPTED'
        return {
          // One row per ATTEMPT: the story id alone would collide across attempts.
          id: `${run.storyId}#${run.attempts}`,
          title: run.title,
          workstream: 'ENGINEERING',
          status: run.status,
          priority: run.status === 'failed' ? 'HIGH' : 'MEDIUM',
          completion: run.status === 'completed' ? 100 : 0,
          // A STALE CLAIM IS NOT WAITING WORK AND NOT LIVE WORK. It is an abandoned attempt: nobody
          // has touched it inside the stale-claim window, so it belongs in RESULTS with its age
          // showing - not in ENGINE QUEUE as "running", which is how an idle engine came to look busy.
          queue: live ? 'running' : 'results',
          outcome: run.stale ? 'INTERRUPTED' : outcome,
          attempt: run.attempts,
          endedOn: run.lastNode ?? undefined,
          storyId: run.storyId,
          instanceId: run.instanceId || undefined,
          ...(run.stale
            ? {
                note: `abandoned claim — no worker since ${
                  run.updatedAt ? run.updatedAt.slice(0, 16) : 'an unrecorded time'
                }`,
              }
            : {}),
        }
      }),
    ],
    [engineRuns, queuedCards],
  )
  // The engine lanes are ENGINE-OWNED: read from the ledger, never moved by hand. There is no local
  // card state any more, because there is no local card data - the fixture is gone.
  const cards = engineCards
  const [selected, setSelected] = useState<string | null>(null)
  // A story on the bench being dragged toward the line: dropping it there hands it to the engine
  // through the real dispatch (see handToEngine).
  const [draggingStory, setDraggingStory] = useState<string | null>(null)
  const [handoffError, setHandoffError] = useState<string | null>(null)
  const [batchResult, setBatchResult] = useState<string | null>(null)
  const [batchSending, setBatchSending] = useState(false)
  const batchCount = (batchStories ?? []).length
  const [moveError, setMoveError] = useState<string | null>(null)
  // The story log is the thing the captain is looking FOR, so it starts open.
  const [showLifecycle, setShowLifecycle] = useState(true)
  // WORKBENCH is one panel now - the human lane, collapsed when you are watching the engine instead.
  const [benchOpen, setBenchOpen] = useState(true)

  // AUTO-REFRESH, because the engine moves whether or not this page is looking.
  //
  // The board is server-rendered: without this it is a photograph. A job queued from GOOD TO GO, a
  // claim that goes stale, a run that finishes - none of it appeared until someone reloaded by hand,
  // which is what made ENGINE QUEUE look sticky (a card that had actually moved on, sitting there as
  // if it had not). 30s is the compromise: fast enough to watch work land, cheap enough to leave open.
  const [refreshedAt, setRefreshedAt] = useState<number | null>(null)
  useEffect(() => {
    const id = setInterval(() => {
      router.refresh()
      setRefreshedAt(Date.now())
    }, 30_000)
    return () => clearInterval(id)
  }, [router])

  function openRecorder(card: QueueCard) {
    // The exact attempt when the ledger knows it; otherwise the recorder resolves the story.
    const target = card.instanceId || card.storyId
    if (!target) return
    window.open(`/portal/tech/flight-recorder/${target}`, '_blank', 'noopener')
  }

  /**
   * THE HANDOFF IS REAL NOW — it was a local demo before.
   *
   * It used to push a card into component state and nothing else happened, so "GOOD TO GO — hand
   * this story to the engine" moved pixels while the engine learned nothing. The real handoff
   * already existed, documented in `app/portal/tech/actions.ts`: moving a story into the ENGINE
   * QUEUE bucket writes `ENGINE_DISPATCH_STATUS` (`Ready`), and `agent_work_item_dispatch()` fires on
   * that change to insert a real work item. It is the only bucket write that STARTS something, it
   * queues real Forge work in PROD, and the SORTER drag already used it — so the button and the drag
   * now go through the same server action, with the same `canMove` rules.
   *
   * Reachable two ways, unchanged: the GOOD TO GO button, or dragging the story onto the line.
   */
  async function handToEngine(storyId: string) {
    setHandoffError(null)
    const result = await moveStoryBucketAction(storyId, bucketForStory(storyId), 'engine')
    if (!result.ok) {
      setHandoffError(result.error ?? 'the engine queue refused this story')
      return
    }
    setSelected(storyId)
    router.refresh()
  }

  /** Which sorter column a story sits in right now, so the move is truthful about its source. */
  function bucketForStory(storyId: string): StoryBucket {
    if (activeWork.some((s) => s.id === storyId)) return 'bench'
    const story = selectedStory?.id === storyId ? selectedStory : null
    const status = story?.status ?? null
    if (!status) return 'open'
    const lifecycle = storyLifecycleOf(status)
    if (lifecycle === 'backlog') return 'backlog'
    if (lifecycle === 'closed') return 'closed'
    if (lifecycle === 'next-version') return 'next'
    return 'open'
  }

  function markGoodToGo() {
    if (!selectedStory) return
    void handToEngine(selectedStory.id)
  }

  // QUEUED is a fact about the engine, not about a card this screen put in state: it reads the open
  // work items (`agent_work_item`) the same way the ENGINE QUEUED lane does.
  const selectedIsQueued = Boolean(
    selectedStory && (queuedCards ?? []).some((q) => q.storyId === selectedStory.id),
  )

  // THE SORTER — the assembly line, left to right in the captain's order:
  //   BACKLOG -> OPEN -> WORK BENCH -> ENGINE QUEUE
  // Different context from the bands below (this is where you SORT, the bands are
  // where you WORK), which is why the same two sets may appear in both without it
  // being a collision. ENGINE QUEUE is empty until agent_work_item is wired.
  const sorterCards = useMemo(() => {
    // `Batched` maps to the backlog LIFECYCLE, so it would otherwise appear in the BACKLOG column too.
    // It has its own column; a story is shown once.
    const bucket = (key: 'backlog' | 'open') =>
      (cockpit.panels[key]?.groups ?? [])
        .flatMap((g) => g.stories)
        .filter((s) => s.status !== 'Batched')
    return [
      ...bucket('backlog').map((s) => ({
        id: s.id,
        column: 'backlog',
        title: s.title,
        status: s.status,
        priority: s.priority,
        completion: s.completion,
      })),
      ...bucket('open').map((s) => ({
        id: s.id,
        column: 'open',
        title: s.title,
        status: s.status,
        priority: s.priority,
        completion: s.completion,
      })),
      ...activeWork.map((s) => ({
        id: s.id,
        column: 'bench',
        title: s.title,
        status: s.status,
        priority: s.priority,
        completion: s.completion,
      })),
      // ENGINE BATCH: staged work, its own column. Batched stories are EXCLUDED from the backlog
      // bucket below (they map to the backlog lifecycle) so the same story is never two columns.
      ...(batchStories ?? []).map((s) => ({
        id: s.id,
        column: 'batch',
        title: s.title,
        status: s.status,
        priority: s.priority,
        completion: s.completion,
      })),
      // NEXT VERSION: the `Deferred` lifecycle. These stories had NO column, so 23 of them (the
      // deferred stories that are not on the bench) could not be reached on this board at all -
      // the second, structural reason "I can't find that story" kept being true.
      ...(cockpit.panels['next-version']?.groups ?? [])
        .flatMap((g) => g.stories)
        .map((s) => ({
          id: s.id,
          column: 'next-version',
          title: s.title,
          status: s.status,
          priority: s.priority,
          completion: s.completion,
        })),
      // ENGINE QUEUE holds what the machine is executing NOW, from the ledger — not a fixture.
      // (It used to be empty by design, "until agent_work_item is wired"; it is wired now, and the
      // real answer since 2026-09-14 is that the engine is idle, which is worth seeing.)
      ...engineCards
        .filter((c) => c.queue === 'running')
        .map((c) => ({
          id: c.id,
          column: 'engine',
          title: c.title,
          status: c.status,
          priority: c.priority,
          completion: c.completion,
        })),
    ]
  }, [cockpit, activeWork, engineCards])

  const sorterColumns = useMemo(
    () => [
      { id: 'backlog', label: 'BACKLOG' },
      { id: 'open', label: 'OPEN' },
      { id: 'bench', label: 'WORK BENCH' },
      // ENGINE BATCH sits directly left of ENGINE QUEUE: stage the next group, then send it.
      { id: 'batch', label: 'ENGINE BATCH' },
      { id: 'engine', label: 'ENGINE QUEUE' },
      { id: 'next-version', label: 'NEXT VERSION' },
    ],
    [],
  )

  const byQueue = (key: QueueKey) => cards.filter((c) => c.queue === key)

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
            <p className="text-[11px] font-semibold tracking-[0.22em] text-[#c6a15b]">TECH / ENGINEERING</p>
            <h1 className="mt-1 font-serif text-2xl font-semibold text-white">Engineering Cockpit</h1>
            <p className="mt-1 max-w-3xl text-sm font-light text-slate-400">
              One screen: what you are working on, what the engine has been given, what it is running,
              and how the last runs ended.
            </p>
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

      {/* SORTER — the assembly line. This is where stories get SORTED; the bands
          below are where the WORK happens, which is why the bench and the engine
          legitimately appear in both (different context, not a collision).
          The drag is local until the writes are wired. */}
      <section className="mb-4">
        <div className="mb-2 flex flex-wrap items-baseline justify-between gap-2">
          <p className="text-[11px] font-semibold tracking-[0.16em] text-white">
            SORTER
            <span className="ml-2 font-normal tracking-[0.08em] text-slate-400">
              backlog → open → work bench → engine batch → engine queue
            </span>
          </p>
          {/*
            SEND THE BATCH — the on-demand dispatch.
            Staging is free and harmless: 'Batched' writes nothing on the engine. This button is the
            act that queues real Forge work for every staged story, so it says how many and what it
            does, and it reports partial success honestly instead of a boolean.
          */}
          <div className="flex items-center gap-2">
            {batchResult ? <span className="text-[10px] text-slate-300">{batchResult}</span> : null}
            <button
              type="button"
              disabled={batchSending || batchCount === 0}
              onClick={async () => {
                setBatchSending(true)
                setBatchResult(null)
                try {
                  const result = await sendEngineBatchAction()
                  setBatchResult(
                    result.failed.length === 0
                      ? `queued ${result.queued} for the engine`
                      : `queued ${result.queued}; refused ${result.failed
                          .map((f) => f.storyId)
                          .join(', ')}`,
                  )
                  router.refresh()
                } finally {
                  setBatchSending(false)
                }
              }}
              title="Queues real Forge work for every story staged in ENGINE BATCH (status → Ready)."
              className="shrink-0 rounded border border-[#c6a15b]/50 bg-[#c6a15b]/15 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.12em] text-[#e0c489] transition hover:bg-[#c6a15b]/25 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {batchSending ? 'Sending…' : `Send batch (${batchCount}) → engine`}
            </button>
          </div>
        </div>
        <div className="mb-2 flex flex-wrap items-baseline justify-between gap-2">
          <p className="text-[10px] text-slate-400">
            drag a story along the line · ENGINE QUEUE fills from the engine
            {engineRuns === null ? (
              <span className="ml-2 text-amber-400/90">
                · the engine ledger could not be read, so the engine lanes are empty rather than
                guessed
              </span>
            ) : engineRuns ? (
              <span className="ml-2 text-slate-500">
                · {engineRuns.length} stor
                {engineRuns.length === 1 ? 'y' : 'ies'} recorded in the ledger
              </span>
            ) : null}
          </p>
        </div>
        <div className="h-[520px] overflow-y-auto rounded-lg border border-white/10 bg-white/[0.02] p-2">
          <StoryKanbanBoard
            cards={sorterCards}
            columns={sorterColumns}
            onMove={async (cardId, from, to) => {
              // The rules live in lib/story-moves.ts; the write lives in the
              // action; a refusal comes back as ok:false and the card snaps back.
              const result = await moveStoryBucketAction(cardId, from, to)
              if (result.ok) router.refresh()
              return result
            }}
          />
        </div>
      </section>

      {/* WORK BENCH — the piece the captain uses most: the queue on the LEFT, the
          selected story's full detail on the RIGHT, run history beneath it. It
          reuses ActiveQueue / StoryDetail / RunHistory from the TECH cockpit, so
          there is exactly ONE implementation of "what the heck is this story". */}
      <section className="mb-4 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-3">
        <div className="mb-2 flex flex-wrap items-baseline justify-between gap-2">
          {/*
            ONE PANEL, COLLAPSIBLE. The Work Bench is the HUMAN lane: the queue, the handoff controls,
            the story detail and its run history. They are all about the same thing - "I am working on
            this story" - so they belong in one panel that folds away when you are watching the engine
            instead of working. Same collapse mechanic as STORY BACKLOG below.
          */}
          <button
            type="button"
            onClick={() => setBenchOpen((v) => !v)}
            className="flex items-baseline gap-2 text-left"
          >
            <span className="text-[11px] font-semibold tracking-[0.16em] text-white">
              WORKBENCH <span className="font-normal text-[#c6a15b]">({activeWork.length})</span>
            </span>
            <span className="ml-1 text-[10px] font-normal tracking-[0.08em] text-slate-400">
              scope today&apos;s work
            </span>
            <span className="text-[11px] text-slate-500">{benchOpen ? '▲' : '▼'}</span>
          </button>
          <p className="text-[10px] text-slate-400">
            my hands, not the engine&apos;s ·{' '}
            <button
              type="button"
              onClick={() => {
                router.refresh()
                setRefreshedAt(Date.now())
              }}
              className="underline decoration-dotted underline-offset-2 hover:text-slate-200"
              title="This board re-reads PROD by itself every 30 seconds; click to do it now."
            >
              refresh now
            </button>
            {refreshedAt ? ' (auto)' : ' (auto every 30s)'}
          </p>
        </div>
        {benchOpen ? (
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
          <ActiveQueue
            activeQueue={activeWork}
            selectedId={selectedStory?.id ?? null}
            basePath="/portal/tech"
            historyStoryIds={historyStoryIds}
            onRowDragStart={setDraggingStory}
            onRowDragEnd={() => setDraggingStory(null)}
          />
          <div className="space-y-3">
            {/* THE GATE. The Work Bench is the human lane (researching today); this is
                where a story stops being mine and becomes the engine's: spec done,
                hand it over. One direction, one click — and it lands in ENGINE QUEUED. */}
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-[#c6a15b]/30 bg-[#c6a15b]/[0.05] px-3 py-2">
              <div className="min-w-0">
                <p className="text-[10px] font-semibold tracking-[0.16em] text-[#e0c489]">GOOD TO GO</p>
                <p className="truncate text-[10px] text-slate-400">
                  {selectedStory
                    ? selectedIsQueued
                      ? `${selectedStory.id} is already waiting for the engine`
                      : `${selectedStory.id} — spec done, hand it to the engine`
                    : 'pick a story on the bench'}
                </p>
              </div>
              <button
                type="button"
                onClick={markGoodToGo}
                title="Queues real Forge work: this writes ENGINE_DISPATCH_STATUS (Ready), which dispatches a work item."
                disabled={!selectedStory || selectedIsQueued}
                className="shrink-0 rounded border border-[#c6a15b]/50 bg-[#c6a15b]/15 px-3 py-1 text-[10px] font-medium uppercase tracking-[0.12em] text-[#e0c489] transition hover:bg-[#c6a15b]/25 disabled:cursor-not-allowed disabled:opacity-40"
              >
                {selectedIsQueued ? 'Queued' : 'Good to go →'}
              </button>
            </div>
            {handoffError ? (
              <p className="rounded border border-rose-400/40 bg-rose-400/10 px-2 py-1 text-[10px] text-rose-200">
                The engine queue refused this story: {handoffError}
              </p>
            ) : null}

            {/* MOVE TO — closing and deferring are DELIBERATE ACTS (they have no
                column to drop on, by design). The rules for what may go where still
                come from lib/story-moves.ts, so this row and the board cannot
                disagree about what is legal. */}
            <div className="flex flex-wrap items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2">
              <span className="mr-1 text-[10px] font-semibold tracking-[0.14em] text-slate-400">
                MOVE TO
              </span>
              {(
                [
                  { to: 'backlog', label: 'Backlog', hint: 'Park it in the backlog' },
                  { to: 'closed', label: 'Closed', hint: 'Finish it — sets completion to 100%' },
                  { to: 'next', label: 'Next Version', hint: 'Defer it to a later version' },
                ] as const
              ).map((target) => (
                <button
                  key={target.to}
                  type="button"
                  title={target.hint}
                  disabled={!selectedStory || !selectedIsActive}
                  onClick={async () => {
                    if (!selectedStory) return
                    // From the BENCH: this control lives in the bench band, so the
                    // source is the bench. Leaving it clears the intent row too.
                    const result = await moveStoryBucketAction(selectedStory.id, 'bench', target.to)
                    setMoveError(result.ok ? null : (result.error ?? 'Refused'))
                    if (result.ok) router.refresh()
                  }}
                  className="rounded border border-white/10 px-2 py-0.5 text-[10px] text-slate-300 transition hover:border-[#c6a15b]/40 hover:text-[#c6a15b] disabled:cursor-not-allowed disabled:opacity-40"
                >
                  → {target.label}
                </button>
              ))}
              {!selectedIsActive ? (
                <span className="text-[10px] text-slate-500">pick a story on the bench</span>
              ) : null}
              {moveError ? <span className="text-[10px] text-rose-300">{moveError}</span> : null}
            </div>
            {selectedStory ? (
              <StoryDetail
                story={selectedStory}
                isActive={selectedIsActive}
                recorderInstanceId={recorderInstanceId ?? null}
              />
            ) : (
              <p className="rounded-lg border border-white/10 bg-white/[0.03] px-4 py-10 text-center text-xs italic text-slate-500">
                Select a story from the Work Bench to inspect it
              </p>
            )}
            {/*
              THE ENGINE'S STOP, ON THE BOARD.
              A parked story used to read "In Progress / 100%" while the machine had deliberately
              stopped and recorded why. The reason is the whole value of a HOLD, so it goes where the
              operator already looks.
            */}
            {hold ? (
              <section className="rounded-lg border border-amber-400/40 bg-amber-400/[0.06] px-4 py-3">
                <p className="text-[10px] font-semibold tracking-[0.14em] text-amber-300">
                  ENGINE HOLD · {hold.originatingNode ?? 'unknown node'}
                  {hold.failureClass ? ` · ${hold.failureClass}` : ''}
                </p>
                <p className="mt-1 text-xs font-light leading-relaxed text-slate-200">
                  {hold.reason ?? 'The engine parked without recording a reason.'}
                </p>
                <p className="mt-1.5 text-[10px] text-slate-400">
                  parked {hold.since ?? 'at an unrecorded time'}
                  {hold.resumeTarget ? ` · resumes at ${hold.resumeTarget}` : ''}
                </p>
              </section>
            ) : null}

            <RunHistory
              storyId={selectedStory?.id ?? null}
              runs={runs}
              withHistory={historyStories}
              historyTotal={historyTotal}
            />
          </div>
        </div>
        ) : null}
      </section>

      {/* ENGINE — the engine's OWN panel: what it has been given, what it is doing, how it ended, and
          the ledger totals for all of it. "Attempts recorded" belongs to the engine, so the stats
          strip lives inside this panel rather than floating between panels. The four panels on this
          screen are SORTER (top), WORKBENCH (human), ENGINE (this one), STORY BACKBOARD (bottom). */}
      <p className="mb-2 text-[11px] font-semibold tracking-[0.16em] text-white">
        ENGINE
        <span className="ml-2 font-normal tracking-[0.08em] text-slate-400">
          handed over → running → results · from agent_work_item and the engine ledger
        </span>
      </p>
      <StatsStrip stats={ledgerStats} />
      <section className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {QUEUES.map((queue) => {
          const items = byQueue(queue.key)
          return (
            <div
              key={queue.key}
              // The only drop that MEANS something on this line: a story dragged off the bench and
              // dropped here is handed to the engine through the real dispatch (`handToEngine` ->
              // ENGINE_DISPATCH_STATUS -> a work item). Dragging one engine card onto another lane
              // used to rewrite local state and nothing else - a mockup of ownership the engine owns -
              // so that gesture is gone.
              onDragOver={(e) => {
                if (draggingStory) e.preventDefault()
              }}
              onDrop={(e) => {
                e.preventDefault()
                if (draggingStory) void handToEngine(draggingStory)
                setDraggingStory(null)
              }}
              className={`flex min-h-[420px] flex-col rounded-lg border ${queue.ring} ${
                draggingStory ? 'bg-white/[0.05] ring-1 ring-[#c6a15b]/40' : 'bg-white/[0.02]'
              }`}
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
        Every number on this board is read from PROD. Tiles and the story log come from one projection
        so they cannot disagree; the Work Bench is{' '}
        <code className="text-slate-400">storyboard_active_work</code> in work order; the engine
        columns come from <code className="text-slate-400">agent_work_item</code> and{' '}
        <code className="text-slate-400">forge_engine_task_execution</code>
        {ledgerStats?.asOf ? ` (as of ${ledgerStats.asOf})` : ''}. GOOD TO GO and a drop on ENGINE
        QUEUE both write the real dispatch status (<code className="text-slate-400">Ready</code>),
        which queues actual Forge work.
      </p>
    </div>
  )
}

function StatsStrip({ stats }: { stats: EngineLedgerStats | null | undefined }) {
  // REAL LEDGER NUMBERS, OR AN HONEST REFUSAL.
  //
  // This strip used to render `model.stats` - fixture figures - under copy that said the numbers were
  // live from PROD. The numbers now come from forge_engine_task_execution, and when that read fails
  // the strip says so instead of showing invented ones.
  if (!stats) {
    return (
      <section className="mb-4 rounded-lg border border-amber-400/30 bg-amber-400/[0.04] px-4 py-3">
        <p className="text-[10px] font-semibold tracking-[0.14em] text-amber-300/90">
          ENGINE LEDGER UNAVAILABLE
        </p>
        <p className="mt-1 text-[11px] font-light text-slate-400">
          The engine ledger could not be read, so these totals are not shown rather than estimated.
        </p>
      </section>
    )
  }

  const latestTotal = stats.latest.completed + stats.latest.failed + stats.latest.interrupted
  const pct = (n: number) =>
    latestTotal === 0 ? '—' : `${Math.round((n / latestTotal) * 100)}%`
  const cells: Array<{ label: string; value: string; tone?: string }> = [
    { label: 'ATTEMPTS RECORDED', value: String(stats.totalAttempts) },
    { label: 'STORIES RUN', value: String(stats.stories) },
    { label: 'LATEST COMPLETE', value: pct(stats.latest.completed), tone: 'text-emerald-300' },
    { label: 'LATEST INTERRUPTED', value: pct(stats.latest.interrupted), tone: 'text-amber-300' },
    { label: 'LATEST FAILED', value: pct(stats.latest.failed), tone: 'text-rose-300' },
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
          <p className="text-[10px] font-semibold tracking-[0.14em] text-slate-400">
            MOST RECORDED ATTEMPTS
          </p>
          <p className="text-xs text-slate-300">
            {stats.worstOffender
              ? `${stats.worstOffender.storyId} · ${stats.worstOffender.attempts} turns`
              : '—'}
          </p>
          <p className="text-[10px] text-slate-500">
            across {stats.stories} stories · ledger as of {stats.asOf ?? '—'}
          </p>
        </div>
      </div>
      <p className="mt-2 text-[11px] font-light text-slate-500">
        Attempts measure churn, not throughput: every role turn the engine dispatches is one row, and a
        story that needs a repair legitimately costs more than one. The last percentages describe each
        story&apos;s MOST RECENT attempt, not its whole history.</p>
    </section>
  )
}

function QueueCardView({
  card,
  selected,
  onSelect,
  onOpen,
}: {
  card: QueueCard
  selected: boolean
  onSelect: () => void
  onOpen: () => void
}) {
  const isResult = card.queue === 'results'
  return (
    // READ-ONLY BY DESIGN. These lanes are the engine's own record of what it was given and what it
    // did; a card here is a FACT, not a draggable thing. (The old cards offered "→ Run" / "→ Results"
    // buttons that rewrote local state and nothing else - moving pixels to look like ownership of a
    // queue the engine owns.) Handing work TO the engine happens in the SORTER's ENGINE QUEUE column
    // or via GOOD TO GO, both of which write the real dispatch status.
    <div
      onClick={onSelect}
      onDoubleClick={isResult ? onOpen : undefined}
      title={isResult ? 'Double-click for the Flight Recorder' : undefined}
      className={`rounded border px-3 py-2 transition ${
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
      {card.note ? (
        <p className="mt-1.5 rounded border border-amber-400/30 bg-amber-400/[0.06] px-1.5 py-0.5 text-[10px] text-amber-200/90">
          {card.note}
        </p>
      ) : null}
    </div>
  )
}

const LIFECYCLE_LABELS: Array<{ key: StoryLifecycle; label: string; long: string }> = [
  // HIS labels first (OPEN / BACKLOG / CLOSED / NEXT VERSION — the 2x2 grid his brain
  // locked onto after two weeks), the descriptive names demoted to the caption so
  // nothing is lost and nothing was silently renamed.
  { key: 'open', label: 'OPEN', long: 'Current Work Queue' },
  { key: 'backlog', label: 'BACKLOG', long: 'Current-Version Waiting' },
  { key: 'closed', label: 'CLOSED', long: 'Finished History' },
  { key: 'next-version', label: 'NEXT VERSION', long: 'Intentionally Future' },
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
        <span className="text-[11px] font-semibold tracking-[0.16em] text-white">
          STORY BACKLOG {open ? '(hide)' : '(show)'}
          <span className="ml-2 font-normal tracking-[0.08em] text-slate-400">open · backlog · closed · next version</span>
        </span>
        <span className="text-[11px] text-slate-500">{open ? '▲' : '▼'}</span>
      </button>
      <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {LIFECYCLE_LABELS.map(({ key, label, long }) => {
          const panel = panels[key]
          return (
            <div key={key} className="flex flex-col rounded border border-white/10">
              <div className="border-b border-white/10 px-3 py-2">
                <p className="font-serif text-sm font-semibold tracking-[0.06em] text-white">{label}</p>
                <p className="text-[10px] text-slate-500">
                  {long} · {panel?.count ?? 0}
                </p>
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

