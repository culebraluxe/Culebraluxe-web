import Link from "next/link"
import { notFound } from "next/navigation"

import { StoryDetailSections } from "@/components/portal/storyboard/story-detail-sections"
import {
  getStoryboardStory,
  listStoryRuns,
} from "@/db/storyboard"
import { workstreamName } from "@/lib/storyboard-data"

export const dynamic = "force-dynamic"

// ---------------------------------------------------------------------------
// Direct story detail route (/portal/storyboard/[id]).
//
// Server-rendered so an external architect/review model (or any reader) can
// open a specific story directly by URL and read its full execution
// specification and run history without relying on chat history or
// client-side expansion. Unknown IDs return a normal not-found.
// ---------------------------------------------------------------------------

export default async function StoryDetailPage({
  params,
}: {
  params: Promise<{ id: string }>
}) {
  const { id } = await params
  const story = await getStoryboardStory(id)

  if (!story) {
    notFound()
  }

  const runs = await listStoryRuns(id)

  return (
    <div>
      <header className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <div className="mt-3 flex flex-wrap items-baseline gap-x-4 gap-y-2">
          <h1 className="font-serif text-4xl font-light leading-[1.1]">
            {story.id}
          </h1>
          <span className="font-serif text-2xl font-light text-black/55">
            {story.title}
          </span>
          <span
            className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-xs font-light ${statusPillClass(
              story.status,
            )}`}
          >
            {story.status}
          </span>
        </div>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          {workstreamName(story.workstream)} · {story.priority} ·{" "}
          {story.completion}% complete · rollup {story.rollup ? "yes" : "no"}
        </p>

        <div className="mt-4 flex flex-wrap items-center gap-3 text-xs font-light">
          <Link
            href="/portal/storyboard"
            className="rounded-sm border border-[var(--portal-border)] px-3 py-1.5 uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
          >
            ← Story Board
          </Link>
          <Link
            href={`/portal/storyboard?q=${encodeURIComponent(story.id)}`}
            className="rounded-sm border border-[var(--portal-border)] px-3 py-1.5 uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
          >
            Open in board
          </Link>
        </div>
      </header>

      <StoryDetailSections story={story} runs={runs} />
    </div>
  )
}

function statusPillClass(status: string): string {
  switch (status) {
    case "Complete":
      return "bg-emerald-50 text-emerald-700"
    case "Blocked":
    case "Failed":
      return "bg-red-50 text-red-700"
    case "In Progress":
    case "Partial":
      return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
    default:
      return "bg-black/5 text-black/55"
  }
}
