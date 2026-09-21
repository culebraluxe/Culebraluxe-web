import { enqueueAppleCalendarCreate } from '@/legacy/db/apple-gateway-outbox'
import { getCatchUpCalendarEvents } from '@/legacy/db/catch-up-calendar'
import type { CalendarRepository } from '@/legacy/services/calendar'
import type { CreateAppleCalendarEventRequest } from '@/legacy/services/calendar'
import type { ServiceContext } from '@/legacy/services/core'

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
