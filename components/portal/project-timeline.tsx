'use client'

import { useEffect, useState } from 'react'
import { Gantt, WillowDark } from '@svar-ui/react-gantt'
import type { ILink, ITask } from '@svar-ui/react-gantt'
import '@svar-ui/react-gantt/all.css'

// ---------------------------------------------------------------------------
// PROJECTS TIMELINE — SVAR Gantt host for the Projects Timeline tab.
//
// A thin host that owns the vendor import, the vendor CSS and the theme wrapper,
// so the pane keeps no widget plumbing (same shape as FullCalendarCandidate).
//
// Two deliberate choices:
//
// - The widget is wrapped in the GANTT's own Willow, not core's. The gantt theme
//   delegates to the core theme and then layers the gantt-specific variables on
//   top (bar colours, timescale fonts, grid borders), so importing core's theme
//   here would drop that layer.
// - The theme's `fonts` prop is left at its DEFAULT. SVAR ships its icon font as
//   part of font loading (there is no wxi @font-face in its CSS — the theme
//   injects it), so disabling fonts risks stripping the widget's icons along with
//   Open Sans. Brand typography is imposed from CSS instead, by overriding
//   --wx-font-family on the theme wrapper, so the pane renders in the portal's
//   serif whether or not the vendor webfont ever arrives.
//
// The skin itself lives in globals.css (.project-timeline). Note that it must
// target the theme wrapper, not just this container: the theme declares all 334
// of its --wx-* variables ON .wx-willow-theme, so an ancestor-only override is
// silently beaten by the theme's own block.
//
// `readonly` is on for now: this is a placeholder surface fed by projected data,
// and an edit made here must not look like it persists. When Timeline edits
// become real they go through the WbsService commands, not through the widget.
// ---------------------------------------------------------------------------

export function ProjectTimeline({ tasks, links }: { tasks: ITask[]; links: ILink[] }) {
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <div className="portal-svar-midnight project-timeline flex h-full min-h-0 w-full flex-col">
      <WillowDark>
        <div className="min-h-[24rem] flex-1 lg:min-h-0">
          {mounted ? (
            <Gantt tasks={tasks} links={links} readonly cellWidth={38} />
          ) : (
            <div className="flex h-full items-center justify-center text-sm font-light text-white/50">
              Loading timeline…
            </div>
          )}
        </div>
      </WillowDark>
    </div>
  )
}
