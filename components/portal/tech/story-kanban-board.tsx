'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import { Kanban, WillowDark } from '@svar-ui/react-kanban'
import type { ColumnConfig, KanbanCard, KanbanInstanceApi } from '@svar-ui/react-kanban'

import '@svar-ui/react-kanban/all.css'

// ---------------------------------------------------------------------------
// STORY KANBAN — SVAR Kanban host. A playground, deliberately thin.
//
// Same shape as components/portal/project-timeline.tsx (the Gantt host): the
// component OWNS the vendor import, the vendor CSS and the theme wrapper, so the
// page keeps no widget plumbing and swapping the widget later touches one file.
//
// Why this vendor and not dnd-kit: @svar-ui/react-kanban is ALREADY a dependency
// of this repo (same family as the Gantt we host), so a draggable board costs no
// new package and no hand-rolled drag. The vendor gives drag between columns and
// reordering inside a column for free.
//
// The drag is LOCAL: the widget mutates its own store, nothing is persisted. That
// is on purpose for a Saturday-night playground — and it is why this page is not
// the cockpit. When the board becomes real, the drop has to write status (and
// sortOrder) through a repository, not through the widget.
// ---------------------------------------------------------------------------

export type StoryKanbanColumn = ColumnConfig
export type StoryKanbanCard = KanbanCard

export function StoryKanbanBoard({
  cards,
  columns,
  onMove,
  onResync,
}: {
  cards: StoryKanbanCard[]
  columns: StoryKanbanColumn[]
  /**
   * The write behind a drop. The board NEVER applies the widget's own move — the vendor rebuilds its
   * whole store whenever the `cards` prop changes (see the interceptor), so a local move would land
   * on a board that has already moved on. The parent writes, then refreshes; the server render is the
   * only thing that moves a card. Return `{ ok: false, error }` and the message is shown.
   */
  onMove?: (
    cardId: string,
    from: string,
    to: string,
  ) => Promise<{ ok: boolean; error?: string; note?: string }>
  /**
   * Called when a write FAILED, so the parent can re-sync the board with the database. Without it a
   * failed write would leave the card drawn where the database does not have it.
   */
  onResync?: () => void
}) {
  const [mounted, setMounted] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // A move can SUCCEED and still have something worth saying — "withdrew 1 queued engine request", "the
  // engine is already running this". That is a fact about what happened, not a refusal, so it is shown
  // as its own line rather than passed off as an error.
  const [notice, setNotice] = useState<string | null>(null)
  // The api is handed to us once by the widget; the store watcher and the header both use it.
  const apiRef = useRef<KanbanInstanceApi | null>(null)

  useEffect(() => {
    setMounted(true)
  }, [])

  // ONE WRITE PATH, two ways in (a drop and a button). The message shown is always the action's own
  // words, so a refusal names its cause whichever gesture produced it.
  const writeMove = useCallback(
    async (cardId: string, from: string, to: string) => {
      if (!onMove) return { ok: true }
      const result = await onMove(cardId, from, to)
      setError(result.ok ? null : (result.error ?? 'Move refused'))
      setNotice(result.ok ? (result.note ?? null) : null)
      return result
    },
    [onMove],
  )

  /**
   * WATCH THE STORE — the write follows the widget's own state, whatever mechanism moved it.
   *
   * Three attempts through the vendor's event plumbing all failed for different reasons: a blocking
   * `intercept` (the card sprang back), an `onMoveCard` prop the vendor routes by convention but does
   * not declare (the card moved on screen and NOTHING reached the database - and the error table was
   * empty, so the handler had simply never run), and `api.on('move-card', …)` which is declared but
   * evidently does not receive the drop either.
   *
   * So this stops trying to catch the event. The widget's own card list IS its state (`api.getCards()`,
   * declared in the vendor's types): whatever moves a card - a drop, a keyboard action, a future vendor
   * code path - the list changes. Polling it and diffing against the last snapshot means the write
   * follows the STATE rather than a callback, which is the only thing three failed attempts have in
   * common. The diff is idempotent: each card's column is written once per change.
   */
  const snapshotRef = useRef<Map<string, string>>(new Map())
  const writtenRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    if (!mounted || !onMove) return
    const tick = () => {
      const api = apiRef.current
      if (!api) return
      let current: KanbanCard[]
      try {
        current = api.getCards() as KanbanCard[]
      } catch {
        return
      }
      const snapshot = new Map<string, string>()
      for (const card of current) snapshot.set(String(card.id), String(card.column ?? ''))
      const previous = snapshotRef.current
      if (previous.size === 0) {
        // First pass is the baseline: the board has just been given its data, nothing has moved yet.
        snapshotRef.current = snapshot
        return
      }
      const now = Date.now()
      for (const [id, column] of snapshot) {
        const before = previous.get(id)
        if (!before || before === column) continue
        // Guard against a write loop: if this exact move was just written, the server data has not
        // come back yet and re-writing it would spin.
        const key = `${id}:${column}`
        const lastWrite = writtenRef.current.get(key) ?? 0
        if (now - lastWrite < 5000) continue
        writtenRef.current.set(key, now)
        void writeMove(id, before, column)
          .then((result) => {
            // A refusal means the database did not accept the move: put the board back rather than let
            // the screen disagree with the row.
            if (!result.ok) onResync?.()
          })
          .catch((error: unknown) => {
            setError(`move failed: ${String((error as Error)?.message ?? error)}`)
            onResync?.()
          })
      }
      snapshotRef.current = snapshot
    }
    const timer = setInterval(tick, 400)
    return () => clearInterval(timer)
  }, [mounted, onMove, writeMove])

  const handleInit = useCallback(
    (api: KanbanInstanceApi) => {
      apiRef.current = api
    },
    [],
  )

  return (
    <div className="portal-svar-midnight story-kanban flex h-full min-h-0 w-full flex-col">
      <WillowDark>
        {/*
          `h-full` here is load-bearing, not cosmetic. The board is `height:100%` and each column
          is `height:100%` with its cards area `flex:1; overflow-y:auto` — so EVERY level between
          the fixed-height panel and the board must have a definite height. This div was
          `flex-1` only (height:auto), which broke the chain: the board sized itself to its
          contents, grew past the panel, and the panel's `overflow-hidden` cut the extra stories
          off with no scrollbar to reach them. `min-h-0` lets it shrink instead of overflowing.
        */}
        <div className="h-full min-h-0 flex-1">
          {mounted ? (
            <Kanban
              cards={cards}
              columns={columns}
              init={handleInit}
              // Which card property decides the column. One string, so the board
              // does not need our model to be reshaped.
              columnAccessor="column"
              cardContent={({ card }) => (
                <div className="px-0.5 py-0.5">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-mono text-[10px] opacity-70">{String(card.id)}</span>
                    <span className="text-[10px] opacity-60">{String(card.status ?? '')}</span>
                  </div>
                  <p className="text-[11px] leading-snug">{String(card.title ?? '')}</p>
                  <p className="text-[10px] opacity-60">
                    {String(card.priority ?? '')} · {Math.round(Number(card.completion ?? 0))}%
                  </p>
                </div>
              )}
            />
          ) : (
            <div className="flex h-96 items-center justify-center text-sm font-light text-slate-400">
              Loading board…
            </div>
          )}
        </div>
      </WillowDark>
      {notice ? (
        <p className="mt-2 rounded border border-[#c6a15b]/30 bg-[#c6a15b]/10 px-2 py-1 text-[11px] text-[#e0c489]">
          {notice}
        </p>
      ) : null}
      {error ? (
        <p className="mt-2 rounded border border-rose-400/30 bg-rose-500/10 px-2 py-1 text-[11px] text-rose-200">
          {error}
        </p>
      ) : null}
    </div>
  )
}
