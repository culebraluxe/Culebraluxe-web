import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { CalendarRepository } from './repository'
import { CALENDAR_OPERATIONS, type CalendarOperationMap } from './types'

/**
 * Calendar service boundary.
 *
 * Apple Calendar remains authoritative for generic schedule events; canonical
 * CulebraLuxe showings stay canonical in their own domain. This service owns the
 * application-facing schedule query and durable outbound command boundary.
 */
export class CalendarService extends BaseService<CalendarOperationMap> {
  readonly domain = 'calendar'
  readonly version = '1'
  readonly description =
    'Reads the normalized schedule and queues explicit Apple Calendar writes through the Mac gateway.'
  protected readonly operations: ServiceOperationDefinitions<CalendarOperationMap>

  constructor(
    private readonly repository: CalendarRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [CALENDAR_OPERATIONS.LIST]: {
        kind: 'query',
        description: 'List the normalized Catch-Up schedule.',
        authorization: 'calendar.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async () => this.repository.list(),
      },
      [CALENDAR_OPERATIONS.CREATE_APPLE_EVENT]: {
        kind: 'command',
        description: 'Queue an explicitly requested Apple Calendar event for the Mac gateway.',
        authorization: 'calendar.write',
        execution: { mode: 'inline' },
        handle: async (request, context) => {
          const title = request.title.trim()
          if (!title) this.fail('CALENDAR_TITLE_REQUIRED', 'Calendar event title is required.')
          const start = new Date(request.startAt)
          const end = new Date(request.endAt)
          if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) {
            this.fail('CALENDAR_TIME_INVALID', 'Calendar event start/end time is invalid.')
          }
          if (end.getTime() <= start.getTime()) {
            this.fail('CALENDAR_END_BEFORE_START', 'Calendar event end must be after its start.')
          }
          return this.repository.createAppleEvent({ ...request, title }, context)
        },
      },
    }
  }

  invariants() {
    return [
      'Apple Calendar remains authoritative for generic schedule events.',
      'Calendar writes are explicit user commands and cross the durable Apple gateway outbox.',
      'Calendar service never converts Apple events into WBS work items.',
    ] as const
  }
}
