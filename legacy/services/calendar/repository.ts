import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'
import type { ServiceContext } from '@/legacy/services/core'
import type { CalendarCommandReceipt, CreateAppleCalendarEventRequest } from '@/legacy/services/calendar/types'

export interface CalendarRepository {
  list(): Promise<CatchUpCalendarEvent[]>
  createAppleEvent(
    request: CreateAppleCalendarEventRequest,
    context: ServiceContext,
  ): Promise<CalendarCommandReceipt>
}
