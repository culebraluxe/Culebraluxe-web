'use client'

import { CatchUpCalendar } from '@/components/portal/catch-up-calendar'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — CAL-02 PRESERVED A/B EVALUATION HARNESS (legacy, NOT rendered).
//
// Lisa selected CALENDAR OPTION B (FullCalendar). The active Catch-Up screen no
// longer renders this harness — it renders <FullCalendarCandidate /> directly.
//
// This file (and its Option A sibling components/portal/catch-up-calendar.tsx)
// is PRESERVED INTACT for reversibility. Option B is NOT rewritten here; it is
// simply promoted to the one visible calendar. Option A's @ilamy/calendar
// source remains so it can be restored later if ever needed.
// ---------------------------------------------------------------------------

export function CatchUpCalendarEvaluation({
  events,
}: {
  events: CatchUpCalendarEvent[]
}) {
  return (
    <div className="flex flex-col gap-3">
      <CatchUpCalendar events={events} heading="Option A" />
      <FullCalendarCandidate events={events} heading="Option B" />
    </div>
  )
}
