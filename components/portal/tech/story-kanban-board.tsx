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
  movesFor,
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
  ) => Promise<{ ok: boolean; error?: string }>
  /**
   * WHERE A CARD MAY GO, offered as BUTTONS on the card.
   *
   * The captain, after fighting the drag: "i dont know why i need all this logic for the Kanban, it should
   * just be simple change the state of story ... this should just update the row in the database."
   *
   * He is right, and this is that. A drag is a gesture routed through the vendor's store, its drop index
   * and its drag state; a BUTTON is one call to one action that writes one row. The button path never
   * touches the vendor, so it cannot be moved onto a rebuilt board, cannot reorder a column it was not
   * aimed at, and cannot pick up a stale index. Same write, same rules, no choreography.
   *
   * Keeping BOTH is deliberate: drag stays for speed, buttons are the path that always means exactly what
   * it says - and if a drag is ever wrong again, there is a way to work that does not depend on it.
   */
  movesFor?: (card: KanbanCard) => Array<{ to: string; label: string; hint?: string }>
}) {
  const [mounted, setMounted] = useState(false)
  const [error, setError] = useState<string | null>(null)
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
      return result
    },
    [onMove],
  )

  const handleInit = useCallback(
    (api: KanbanInstanceApi) => {
      apiRef.current = api
      if (!onMove) return
      // INTERCEPT, not ON: intercept can return a Promise<boolean>, so the widget
      // waits for the write and cancels the move when it fails. That is the
      // rollback — no optimistic state to unwind by hand.
      // THE SERVER OWNS POSITION. The widget's own move is never applied, and `false` is returned
      // even on SUCCESS.
      //
      // WHY, from the vendor's own source (`components/Kanban.jsx`):
      //
      //     useEffect(() => { store.init({ cards, columns, ... }) }, [cards, columns, ...])
      //
      // EVERY NEW `cards` PROP REBUILDS THE WIDGET'S ENTIRE STORE. So a drop went: write -> our
      // `router.refresh()` -> new `cards` -> store re-init with the story ALREADY moved -> and only
      // then did this interceptor resolve `true`, letting the widget apply the move it had queued
      // against the REBUILT board, where the drop index points at a DIFFERENT card. The captain saw
      // it exactly: "i move the story left to right and it pulls some adjacent story to the right."
      // The store never needed reverting (the move handler never ran), so cancelling is clean: the
      // server render that follows is the only thing that moves a card.
      //
      // Verified against the library, not guessed: there is no WIP limit, no column balancing and no
      // `limit` prop anywhere in @svar-ui/react-kanban - this was our refresh racing the widget.
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
        if (!to || from === to) return false
        await writeMove(id, from, to)
        return false
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
              cardContent={({ card }) => {
                // The buttons are the deterministic path: one action call, one row written, and the
                // server render is the only thing that moves a card (see `movesFor`).
                const targets = movesFor?.(card) ?? []
                return (
                  <div className="px-0.5 py-0.5">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-mono text-[10px] opacity-70">{String(card.id)}</span>
                      <span className="text-[10px] opacity-60">{String(card.status ?? '')}</span>
                    </div>
                    <p className="text-[11px] leading-snug">{String(card.title ?? '')}</p>
                    <p className="text-[10px] opacity-60">
                      {String(card.priority ?? '')} · {Math.round(Number(card.completion ?? 0))}%
                    </p>
                    {targets.length ? (
                      <div className="mt-1.5 flex flex-wrap gap-1">
                        {targets.map((t) => (
                          <button
                            key={t.to}
                            type="button"
                            // The vendor owns pointer events on the card; a button must not start a drag
                            // or open the card popup, so the event stops here.
                            onPointerDown={(e) => e.stopPropagation()}
                            onMouseDown={(e) => e.stopPropagation()}
                            onClick={(e) => {
                              e.stopPropagation()
                              e.preventDefault()
                              void writeMove(String(card.id), String(card.column ?? ''), t.to)
                            }}
                            title={t.hint ?? `Move ${String(card.id)} to ${t.label}`}
                            className="rounded border border-[#c6a15b]/40 px-1.5 py-[1px] text-[9px] font-medium uppercase tracking-[0.08em] text-[#e0c489] transition hover:border-[#c6a15b] hover:bg-[#c6a15b]/20"
                          >
                            → {t.label}
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                )
              }}
            />
          ) : (
            <div className="flex h-96 items-center justify-center text-sm font-light text-slate-400">
              Loading board…
            </div>
          )}
        </div>
      </WillowDark>
      {error ? (
        <p className="mt-2 rounded border border-rose-400/30 bg-rose-500/10 px-2 py-1 text-[11px] text-rose-200">
          {error}
        </p>
      ) : null}
    </div>
  )
}
