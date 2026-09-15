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
   * WATCH THE STORE'S OWN LAYOUT — the write follows the widget's state, whatever moved it.
   *
   * Four attempts failed before this one. Three hooked the vendor's event plumbing (a blocking
   * `intercept`, an undeclared `onMoveCard` prop, then the declared `api.on('move-card')`) and none of
   * them ever ran: the card moved on screen, nothing reached the database, and the error table stayed
   * empty. The fourth diffed `api.getCards()` - which returns the CARDS STORE, the original flat input,
   * not the layout the operator is looking at.
   *
   * THE AUTHORITATIVE LAYOUT, from the vendor's own store types (`@svar-ui/kanban-store`):
   *
   *     State = { ..., viewData: { columns: ColumnView[] } }
   *     ColumnView = { id, label, ..., cards: KanbanCard[] }
   *
   * So the live board is `state.viewData.columns[].cards[]`: which column holds which cards RIGHT NOW.
   * Diffing that against the previous pass means the write follows the state, not a callback - a drop, a
   * keyboard move, or any future vendor mechanism is all the same to this code. Each change is written
   * once (5s guard against a write loop while the server data catches up).
   */
  const snapshotRef = useRef<Map<string, string>>(new Map())
  const writtenRef = useRef<Map<string, number>>(new Map())
  const announcedRef = useRef(false)

  useEffect(() => {
    if (!mounted || !onMove) return

    const readLayout = (): Map<string, string> | null => {
      const api = apiRef.current
      if (!api) return null
      let state: {
        viewData?: { columns?: Array<{ id?: unknown; cards?: Array<{ id?: unknown }> }> }
      }
      try {
        state = api.getState() as typeof state
      } catch {
        return null
      }
      const columns = state?.viewData?.columns ?? []
      const layout = new Map<string, string>()
      for (const column of columns) {
        const columnId = String(column?.id ?? '')
        for (const card of column?.cards ?? []) layout.set(String(card?.id), columnId)
      }
      return layout
    }

    const trace = (payload: Record<string, unknown>) => {
      // Fire-and-forget: the trace must never affect the board.
      void fetch('/api/portal/move-trace', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload),
        keepalive: true,
      }).catch(() => {})
    }

    const tick = () => {
      const layout = readLayout()
      if (!layout || layout.size === 0) return

      if (!announcedRef.current) {
        announcedRef.current = true
        // "the watcher is alive and this is what it can see" - the fact that was missing every time.
        trace({ phase: 'watcher-alive', detail: `cards=${layout.size}` })
      }

      const previous = snapshotRef.current
      if (previous.size === 0) {
        snapshotRef.current = layout
        return
      }
      const now = Date.now()
      for (const [id, column] of layout) {
        const before = previous.get(id)
        if (!before || before === column) continue
        const key = `${id}:${column}`
        if (now - (writtenRef.current.get(key) ?? 0) < 5000) continue
        writtenRef.current.set(key, now)
        trace({ phase: 'observed-move', cardId: id, from: before, to: column })
        void writeMove(id, before, column)
          .then((result) => {
            if (!result.ok) onResync?.()
          })
          .catch((error: unknown) => {
            setError(`move failed: ${String((error as Error)?.message ?? error)}`)
            onResync?.()
          })
      }
      snapshotRef.current = layout
    }

    const timer = setInterval(tick, 400)
    return () => clearInterval(timer)
  }, [mounted, onMove, onResync, writeMove])

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
