import type { CatchUpCalendarEvent } from '../../lib/catchup/calendar-adapter'

export type CreateAppleCalendarEventRequest = {
  title: string
  startAt: string
  endAt: string
  allDay?: boolean
  location?: string | null
  notes?: string | null
  alert?: boolean
}

export type CalendarCommandReceipt = {
  commandId: string
  state: 'queued'
}

export const CALENDAR_OPERATIONS = {
  LIST: 'calendar.list',
  CREATE_APPLE_EVENT: 'calendar.createAppleEvent',
} as const

export type CalendarOperationMap = {
  'calendar.list': { request: Record<string, never>; response: CatchUpCalendarEvent[] }
  'calendar.createAppleEvent': {
    request: CreateAppleCalendarEventRequest
    response: CalendarCommandReceipt
  }
}
