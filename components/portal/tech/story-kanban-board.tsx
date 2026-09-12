'use client'

import { useEffect, useState } from 'react'
import { Kanban, WillowDark } from '@svar-ui/react-kanban'
import type { ColumnConfig, KanbanCard } from '@svar-ui/react-kanban'

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
}: {
  cards: StoryKanbanCard[]
  columns: StoryKanbanColumn[]
}) {
  const [mounted, setMounted] = useState(false)

  // The widget touches the DOM on mount; render it only after mount, exactly as
  // the Gantt host does, so the server pass stays clean.
  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <div className="story-kanban flex h-full min-h-0 w-full flex-col">
      <WillowDark>
        <div className="flex-1">
          {mounted ? (
            <Kanban
              cards={cards}
              columns={columns}
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
    </div>
  )
}
