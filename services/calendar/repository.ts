import type { CatchUpCalendarEvent } from '../../lib/catchup/calendar-adapter'
import type { ServiceContext } from '../core'
import type { CalendarCommandReceipt, CreateAppleCalendarEventRequest } from './types'

export interface CalendarRepository {
  list(): Promise<CatchUpCalendarEvent[]>
  createAppleEvent(
    request: CreateAppleCalendarEventRequest,
    context: ServiceContext,
  ): Promise<CalendarCommandReceipt>
}
