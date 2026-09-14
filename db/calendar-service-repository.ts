import { enqueueAppleCalendarCreate } from './apple-gateway-outbox'
import { getCatchUpCalendarEvents } from './catch-up-calendar'
import type { CalendarRepository } from '../services/calendar'
import type { CreateAppleCalendarEventRequest } from '../services/calendar'
import type { ServiceContext } from '../services/core'

/** Infrastructure adapter for CalendarService. */
export class SqlCalendarRepository implements CalendarRepository {
  async list() {
    return getCatchUpCalendarEvents()
  }

  async createAppleEvent(
    request: CreateAppleCalendarEventRequest,
    context: ServiceContext,
  ) {
    const commandId = await enqueueAppleCalendarCreate(request, {
      actorAppUserId: context.principal?.appUserId ?? context.actor.id,
      correlationId: context.correlationId,
    })
    return { commandId, state: 'queued' as const }
  }
}
