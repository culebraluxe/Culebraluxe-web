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
}) {
  const [mounted, setMounted] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // A move can SUCCEED and still have something worth saying — "withdrew 1 queued engine request", "the
  // engine is already running this". That is a fact about what happened, not a refusal, so it is shown
  // as its own line rather than passed off as an error.
  const [notice, setNotice] = useState<string | null>(null)
  // Latest cards for the intercept handler: it runs outside React's render, so a
  // closure over props would go stale after the first server refresh.
  const cardsRef = useRef(cards)
  // The api is handed to us once by the widget; keep it so the header can use it.
  const apiRef = useRef<KanbanInstanceApi | null>(null)

  useEffect(() => {
    setMounted(true)
  }, [])

  useEffect(() => {
    cardsRef.current = cards
  }, [cards])

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

  const handleInit = useCallback(
    (api: KanbanInstanceApi) => {
      apiRef.current = api
      if (!onMove) return
      // INTERCEPT, not ON: intercept can return a Promise<boolean>, so the widget waits for the write
      // and puts the card back when it fails.
      //
      // LET THE WIDGET MOVE THE CARD — the drop STAYS where it was put.
      //
      // This is the fix for the captain's report: "i let go and the story immediately falls back to its
      // original state". The interceptor used to return `false` even on success, which CANCELS the
      // widget's own move, and the card was supposed to reappear in the new column from the server
      // render - it did not, because the vendor's store keeps its own copy of each card, so cancelling
      // the local move left the node in its OLD lane. That is the opposite of the demo behaviour he
      // described: "you can move a node from one lane to another lane, it removes it from old lane and
      // adds it to new lane". He is right: the widget's own move is the interaction, and the database
      // write is the side effect.
      //
      //   `true`  -> the write SUCCEEDED, so the widget keeps its move (the card stays in the new lane).
      //   `false` -> the write FAILED, or the move is ambiguous, so the widget puts the card back.
      //
      // THE OTHER HALF OF THE OLD BUG, handled at the other end: re-initialising the vendor's store
      // mid-drop is what once dragged an ADJACENT card into the wrong lane ("i move the story left to
      // right and it pulls some adjacent story to the right"). The parent therefore does NOT refresh
      // during the drop - it refreshes after the move has settled (see `onMove` in the sorter).
      void api.intercept('move-card', async (data) => {
        const id = String(data.id)
        const to = String(data.column ?? '')
        // THE SOURCE MUST BE DERIVED, and the vendor cannot help: `move-card` carries only
        // `{ id, column, before }`, so the origin column has to come from our model. That derivation
        // is only sound when a card id appears in ONE column - and it silently did not: a bench story
        // also sat in OPEN (the bench is an intent row, the status stays `In Progress`), so `find`
        // returned the OPEN card and every drag off the bench reported `from='open'`. Bench -> Open
        // then read as a no-op and Bench -> Batch staged the story without clearing the bench.
        const matches = cardsRef.current.filter((c) => String(c.id) === id)
        const columnsForCard = [...new Set(matches.map((c) => String(c.column ?? '')))]
        if (columnsForCard.length > 1) {
          // A card in two columns is ambiguous. Refuse it visibly rather than move the wrong story.
          setError(
            `Board error: ${id} appears in ${columnsForCard.join(' and ')}. One story, one column.`,
          )
          return false
        }
        const from = columnsForCard[0] ?? ''
        if (!to) return false
        // Reordering inside one column is not a status change and is not persisted; let it happen.
        if (from === to) return true
        const result = await writeMove(id, from, to)
        return result.ok
      })
    },
    [onMove, writeMove],
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
