import { normalizeInboundEvent } from './crm-intake-normalization'
import { prepareInboundEvent } from './crm-intake'
import { resolveOrCreateInboundPerson } from './crm-person-creation'
import type { IntakeRepositories } from './crm-intake-types'
import type { PersonCreationRepositories } from './crm-person-types'
import { adaptCalendarEvent } from './crm-calendar-normalization'
import type {
  CalendarAdapterConfiguration,
  CalendarIntakeResult,
  CalendarProviderEvent,
} from './crm-calendar-types'

export interface CalendarIntakeRepositories
  extends IntakeRepositories,
    PersonCreationRepositories {}

/**
 * CRM-08 readiness coordinator. Duplicate-first source identity, exact CRM-02
 * context resolution, and CRM-03 person resolution — unchanged contracts.
 *
 * Calendar readiness scaffolding does not create persons. Provider
 * authentication verifies the owned calendar transport/account, not ownership
 * of an external attendee email/phone, so attendee identity is always
 * `user_supplied` and person auto-creation is never enabled here. Stronger
 * attendee assurance would require a separately reviewed future capability.
 *
 * This returns canonical in-memory inputs only and never writes an
 * interaction, task, interest, or person. No requested action is inferred,
 * so no follow-up task noise is derived from a calendar event. The LIVE
 * durability layer (lib/calendar/lowering.ts + calendar_intake_receipt,
 * migration 040) composes this coordinator with a provider adapter
 * (lib/calendar/google) and persists only 'ready' outcomes.
 */
export async function prepareCalendarIntake(
  event: CalendarProviderEvent,
  configuration: CalendarAdapterConfiguration,
  repositories: CalendarIntakeRepositories,
): Promise<CalendarIntakeResult> {
  const adapted = adaptCalendarEvent(event, configuration)
  if (adapted.status !== 'accepted') return adapted

  const normalizedEvent = normalizeInboundEvent(adapted.inboundEvent)
  const duplicate = await repositories.findInteractionBySourceIdentity(
    normalizedEvent.source.system,
    normalizedEvent.source.externalId,
  )
  if (duplicate) {
    return { status: 'duplicate', existingInteractionId: duplicate.id }
  }

  // Calendar attendee identity is organizer-supplied and never ownership-
  // verified, so the calendar adapter must never enable CRM-03 auto-creation.
  // allowCreation stays false for every calendar-derived external actor.
  const allowCreation = false
  const personResult = await resolveOrCreateInboundPerson(
    normalizedEvent,
    { allowCreation, role: adapted.applicableCreationRole ?? 'buyer' },
    repositories,
  )
  if (personResult.status === 'duplicate' && personResult.existingInteractionId) {
    return {
      status: 'duplicate',
      existingInteractionId: personResult.existingInteractionId,
    }
  }
  if (
    personResult.status !== 'created' &&
    personResult.status !== 'resolved_existing'
  ) {
    return {
      status:
        personResult.status === 'rejected' || personResult.status === 'conflicting'
          ? 'rejected'
          : 'resolution_required',
      reason: personResult.reason ?? personResult.status,
      personResult,
    }
  }

  const intakeResult = await prepareInboundEvent(
    {
      ...adapted.inboundEvent,
      actor: {
        ...adapted.inboundEvent.actor,
        personId: personResult.personId,
      },
    },
    repositories,
  )
  if (intakeResult.status === 'duplicate' && intakeResult.existingInteractionId) {
    return {
      status: 'duplicate',
      existingInteractionId: intakeResult.existingInteractionId,
    }
  }
  if (intakeResult.status !== 'ready') {
    return {
      status:
        intakeResult.status === 'resolution_required'
          ? 'resolution_required'
          : 'rejected',
      reason: `crm_intake_${intakeResult.status}`,
      personResult,
    }
  }
  return { status: 'ready', personResult, intakeResult }
}
