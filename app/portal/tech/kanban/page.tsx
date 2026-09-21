import { redirect } from "next/navigation"

import { StoryKanbanBoard } from "@/components/portal/tech/story-kanban-board"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import { listStoryExecutionSummaries, listStoryboardStories } from "@/legacy/db/storyboard"
import { buildStoryBoardCockpit, buildStoryBoardModel } from "@/lib/storyboard-data"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// KANBAN PLAYGROUND — the SVAR board with REAL stories in three lists.
//
// Not the cockpit, and deliberately not linked from it: this exists to answer one
// question with the eyes rather than in the abstract — does the vendor board feel
// right for this? The columns are OPEN / BACKLOG / CLOSED fed from the same
// projection the cockpit uses, so the cards are real stories, not lorem ipsum.
//
// Dragging is LOCAL (the widget's own store). Moving a card here is a gesture, not
// a write. When this becomes real the drop must write status + sortOrder through a
// repository, and the legal transitions belong in data (canMove), not in the widget.
const COLUMNS = [
  { id: "open", label: "OPEN" },
  { id: "backlog", label: "BACKLOG" },
  { id: "closed", label: "CLOSED" },
] as const

/** Bound the page: a few dozen cards per column is plenty to judge the feel. */
const PER_COLUMN = 40

export default async function KanbanPlaygroundPage() {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const [stories, executions] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
  ])
  if (!stories) return <StoryBoardNotReady />

  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))
  const cockpit = buildStoryBoardCockpit(buildStoryBoardModel(withExecution))

  const cards = COLUMNS.flatMap((column) => {
    const panel = cockpit.panels[column.id]
    const bucket = (panel?.groups ?? []).flatMap((g) => g.stories)
    return bucket.slice(0, PER_COLUMN).map((s) => ({
      id: s.id,
      // The property `columnAccessor="column"` reads.
      column: column.id,
      title: s.title,
      status: s.status,
      priority: s.priority,
      completion: s.completion,
      workstream: String(s.workstream ?? ""),
    }))
  })

  return (
    <div className="min-h-screen bg-[#0b1220] px-5 py-6 text-slate-200">
      <header className="mb-4">
        <p className="text-[11px] font-semibold tracking-[0.22em] text-[#c6a15b]">TECH / PLAYGROUND</p>
        <h1 className="mt-1 font-serif text-2xl font-semibold text-white">Story Kanban</h1>
        <p className="mt-1 max-w-3xl text-sm font-light text-slate-400">
          @svar-ui/react-kanban, three lists, real stories from the board. Drag between the lists and
          reorder inside one — the movement is local to this page and writes nothing.
        </p>
        <p className="mt-1 text-[11px] text-slate-500">
          {cards.length} cards · OPEN {COLUMNS[0].label} / BACKLOG / CLOSED · first {PER_COLUMN} per list
        </p>
      </header>
      <StoryKanbanBoard cards={cards} columns={COLUMNS.map((c) => ({ id: c.id, label: c.label }))} />
    </div>
  )
}
