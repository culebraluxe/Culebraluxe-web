'use client'

import { useEffect, useMemo, useState } from 'react'
import { createPortal } from 'react-dom'

import type { ILink, ITask } from '@svar-ui/react-gantt'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import { ProjectTimeline } from '@/components/portal/project-timeline'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

type TimelinePayload = {
  tasks: Array<{
    id: number
    text: string
    type?: string
    parent?: number
    open?: boolean
    progress?: number
    start?: string
    duration?: number
    details?: string
  }>
  links: ILink[]
}

type CalendarPayload = {
  events: CatchUpCalendarEvent[]
}

type IslandState = {
  timelineTarget: Element | null
  timeline: TimelinePayload | null
  calendarTarget: Element | null
  calendar: CalendarPayload | null
}

const EMPTY: IslandState = {
  timelineTarget: null,
  timeline: null,
  calendarTarget: null,
  calendar: null,
}

function readPayload<T>(element: Element | null): T | null {
  const raw = element?.getAttribute('data-project-widget')
  if (!raw) return null
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

function sameIslandState(left: IslandState, right: IslandState): boolean {
  return (
    left.timelineTarget === right.timelineTarget &&
    left.calendarTarget === right.calendarTarget &&
    JSON.stringify(left.timeline) === JSON.stringify(right.timeline) &&
    JSON.stringify(left.calendar) === JSON.stringify(right.calendar)
  )
}

/**
 * Rendering adapter only.
 *
 * Yew owns the page, project selection, tabs, and commands. This component
 * observes Yew's two vendor slots and portals the existing React-only widgets
 * into them. No application state crosses back from these read-only widgets.
 */
export function ProjectReactIslands() {
  const [state, setState] = useState<IslandState>(EMPTY)

  useEffect(() => {
    const root = document.getElementById('rust-ui')
    if (!root) return

    const scan = () => {
      const timelineTarget = root.querySelector('#project-timeline-island')
      const calendarTarget = root.querySelector('#project-calendar-island')
      const next: IslandState = {
        timelineTarget,
        timeline: readPayload<TimelinePayload>(timelineTarget),
        calendarTarget,
        calendar: readPayload<CalendarPayload>(calendarTarget),
      }
      setState((current) => (sameIslandState(current, next) ? current : next))
    }

    scan()
    const observer = new MutationObserver(scan)
    observer.observe(root, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-project-widget'],
    })
    return () => observer.disconnect()
  }, [])

  const timelineTasks = useMemo<ITask[]>(() => {
    return (state.timeline?.tasks ?? []).map((task) => ({
      ...task,
      ...(task.start
        ? {
            // Due dates are calendar dates. Local noon avoids a UTC-midnight
            // shift to the prior day in Puerto Rico.
            start: new Date(`${task.start}T12:00:00`),
          }
        : {}),
    })) as ITask[]
  }, [state.timeline])

  return (
    <>
      {state.timelineTarget && state.timeline
        ? createPortal(
            <ProjectTimeline tasks={timelineTasks} links={state.timeline.links ?? []} />,
            state.timelineTarget,
          )
        : null}
      {state.calendarTarget && state.calendar
        ? createPortal(
            <FullCalendarCandidate events={state.calendar.events ?? []} heading={null} />,
            state.calendarTarget,
          )
        : null}
    </>
  )
}
