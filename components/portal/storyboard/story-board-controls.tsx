"use client"

import { useRouter } from "next/navigation"
import { useState } from "react"

import {
  STORY_PRIORITIES,
  STORY_STATUSES,
  WORKSTREAMS,
  OPERATING_SURFACES,
  isStoryBoardFilterActive,
  storyBoardFilterToQuery,
  type StoryBoardFilter,
} from "@/lib/storyboard-data"

// ---------------------------------------------------------------------------
// Story Board search + filter controls.
//
// Pure client controls: every change is reflected into the URL query string
// (stable names: q, workstream, status, priority, view, rollup) so filtered
// views are directly linkable, bookmarkable, refresh-safe, and server-visible.
// The authoritative dashboard and rollup math are unaffected — this controls
// only which stories are shown beneath the dashboard.
// ---------------------------------------------------------------------------

const selectClass =
  "min-h-11 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"

const inputClass =
  "min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"

const clearButtonClass =
  "min-h-11 inline-flex items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"

const labelClass = "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"

export function StoryBoardControls({
  filter,
  visibleCount,
  totalCount,
}: {
  filter: StoryBoardFilter
  visibleCount: number
  totalCount: number
}) {
  const router = useRouter()
  const [query, setQuery] = useState(filter.q)

  function navigate(next: StoryBoardFilter) {
    const qs = storyBoardFilterToQuery(next)
    router.replace(qs ? `/portal/storyboard?${qs}` : "/portal/storyboard", {
      scroll: false,
    })
  }

  function setPart<K extends keyof StoryBoardFilter>(
    key: K,
    value: StoryBoardFilter[K],
  ) {
    navigate({ ...filter, [key]: value })
  }

  function clearAll() {
    setQuery("")
    router.replace("/portal/storyboard", { scroll: false })
  }

  const active = isStoryBoardFilterActive(filter)

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 className="font-serif text-2xl font-light">Search &amp; Filter</h2>
            <p className="mt-1 text-sm font-light text-black/50">
              Narrow the story list below; the executive dashboard always shows
              the authoritative whole-project state.
            </p>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-xs font-light uppercase tracking-[0.18em] text-black/40">
              Showing {visibleCount} of {totalCount} stories
            </span>
            <button
              type="button"
              disabled={!active && !query.trim()}
              onClick={clearAll}
              className={clearButtonClass}
            >
              Clear Filters
            </button>
          </div>
        </div>

        <div className="mt-5 grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
          <label className="block lg:col-span-2 xl:col-span-1">
            <span className={labelClass}>Search</span>
            <input
              type="search"
              value={query}
              placeholder="ID, title, notes, architect brief, acceptance criteria…"
              onChange={(event) => {
                const value = event.target.value
                setQuery(value)
                navigate({ ...filter, q: value })
              }}
              className={`${inputClass} mt-2`}
            />
          </label>

          <label className="block">
            <span className={labelClass}>Workstream</span>
            <select
              value={filter.workstream}
              onChange={(event) =>
                setPart(
                  "workstream",
                  event.target.value as StoryBoardFilter["workstream"],
                )
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All workstreams</option>
              {WORKSTREAMS.map((ws) => (
                <option key={ws.code} value={ws.code}>
                  {ws.name} ({ws.code})
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Operating surface</span>
            <select
              value={filter.surface}
              onChange={(event) =>
                setPart(
                  "surface",
                  event.target.value as StoryBoardFilter["surface"],
                )
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All surfaces</option>
              {OPERATING_SURFACES.map((surface) => (
                <option key={surface} value={surface}>
                  {surface}
                </option>
              ))}
              <option value="unclassified">Unclassified (NULL)</option>
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Status</span>
            <select
              value={filter.status}
              onChange={(event) =>
                setPart(
                  "status",
                  event.target.value as StoryBoardFilter["status"],
                )
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All statuses</option>
              {STORY_STATUSES.map((status) => (
                <option key={status} value={status}>
                  {status}
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Priority</span>
            <select
              value={filter.priority}
              onChange={(event) =>
                setPart(
                  "priority",
                  event.target.value as StoryBoardFilter["priority"],
                )
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All priorities</option>
              {STORY_PRIORITIES.map((priority) => (
                <option key={priority} value={priority}>
                  {priority}
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Work view</span>
            <select
              value={filter.view}
              onChange={(event) =>
                setPart("view", event.target.value as StoryBoardFilter["view"])
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All</option>
              <option value="open">Open Work</option>
              <option value="blocked-failed">Blocked / Failed</option>
              <option value="complete">Complete</option>
              <option value="deferred-hold">Deferred / Hold</option>
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Rollup</span>
            <select
              value={filter.rollup}
              onChange={(event) =>
                setPart(
                  "rollup",
                  event.target.value as StoryBoardFilter["rollup"],
                )
              }
              className={`${selectClass} mt-2 w-full`}
            >
              <option value="all">All</option>
              <option value="in">Counts in rollup</option>
              <option value="out">Parent / non-rollup only</option>
            </select>
          </label>
        </div>
      </div>
    </section>
  )
}

