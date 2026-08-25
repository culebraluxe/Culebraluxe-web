'use client'

import { CatchUpCalendar } from '@/components/portal/catch-up-calendar'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — CAL-02 temporary dual-calendar A/B (ilamy vs FullCalendar).
//
// Both candidates consume the SAME normalized events and open on the SAME
// month, stacked full-width so Lisa compares engines fairly. Labels are neutral
// (OPTION A / OPTION B) — pricing is discussed only after she chooses. After
// selection, delete the losing candidate component and render the winner here.
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
