import { CatchUp } from '@/components/portal/catch-up'
import { getCatchUpEligiblePage } from '@/db/catch-up'
import { getCatchUpTasks } from '@/db/tasks'
import { buildCatchUpQueue } from '@/lib/catchup/queue'
import { getCatchUpCalendarEvents } from '@/db/catch-up-calendar'
import { getEvaluationCalendarEvents } from '@/lib/catchup/eval-data'

export const dynamic = 'force-dynamic'

// CATCH-UP — CORE destination. Server-side initial queue + task tree + calendar
// projection; the queue re-pages client-side over the bounded /api/portal/catch-up
// read. The task tree reads the canonical public.task rows (workstream taxonomy).
// CAL-02: real showings + deterministic evaluation events are combined ONCE
// into a single normalized array that BOTH calendar candidates consume.
export default async function CatchUpPage() {
  const [{ rows }, tasks, realEvents] = await Promise.all([
    getCatchUpEligiblePage({ page: 1, pageSize: 50 }),
    getCatchUpTasks(),
    getCatchUpCalendarEvents(),
  ])

  const calendarEvents = [...realEvents, ...getEvaluationCalendarEvents()]

  const queue = buildCatchUpQueue(rows)
  const newLeads = queue.filter((i) => i.reasonCode === 'new_lead').length
  const needsResponse = queue.filter(
    (i) => i.reasonCode === 'needs_response',
  ).length

  const parts: string[] = []
  if (queue.length > 0) parts.push(`${queue.length} need attention`)
  if (newLeads > 0) parts.push(`${newLeads} new lead${newLeads === 1 ? '' : 's'}`)
  if (needsResponse > 0)
    parts.push(`${needsResponse} need${needsResponse === 1 ? 's' : ''} a response`)
  if (calendarEvents.length > 0)
    parts.push(`Calendar · ${calendarEvents.length} upcoming`)

  const statusCue =
    parts.length > 0 ? parts.join(' · ') : 'No urgent follow-ups'

  return (
    <CatchUp
      calendarEvents={calendarEvents}
      statusCue={statusCue}
      tasks={tasks}
    />
  )
}
