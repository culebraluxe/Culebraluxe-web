import { normalizeInboundEvent } from './crm-intake-normalization'
import { prepareInboundEvent } from './crm-intake'
import { resolveOrCreateInboundPerson } from './crm-person-creation'
import type { IntakeRepositories } from './crm-intake-types'
import type { PersonCreationRepositories } from './crm-person-types'
import { adaptWhatsAppEvent } from './crm-whatsapp-normalization'
import type {
  WhatsAppAdapterConfiguration,
  WhatsAppIntakeResult,
  WhatsAppProviderEvent,
} from './crm-whatsapp-types'

export interface WhatsAppIntakeRepositories
  extends IntakeRepositories,
    PersonCreationRepositories {}

/**
 * CRM-07 readiness coordinator. Duplicate-first source identity, exact CRM-02
 * context resolution, and CRM-03 person resolution — unchanged contracts.
 *
 * WhatsApp actor assurance is `transport_observed` only (a signed webhook
 * proves delivery integrity, not ownership), so attendee/actor identity is
 * always `user_supplied` and person auto-creation is never enabled here.
 *
 * This returns canonical in-memory inputs only and never writes an
 * interaction, task, interest, or person. No requested action is inferred, so
 * no follow-up task noise is derived. Live ingestion additionally requires a
 * reviewed provider connector and a durable receipt/cursor boundary before any
 * persistence is authorized.
 */
export async function prepareWhatsAppIntake(
  event: WhatsAppProviderEvent,
  configuration: WhatsAppAdapterConfiguration,
  repositories: WhatsAppIntakeRepositories,
): Promise<WhatsAppIntakeResult> {
  const adapted = adaptWhatsAppEvent(event, configuration)
  if (adapted.status !== 'accepted') return adapted

  const normalizedEvent = normalizeInboundEvent(adapted.inboundEvent)
  const duplicate = await repositories.findInteractionBySourceIdentity(
    normalizedEvent.source.system,
    normalizedEvent.source.externalId,
  )
  if (duplicate) {
    return { status: 'duplicate', existingInteractionId: duplicate.id }
  }

  // WhatsApp actor identity is never ownership-verified, so the adapter must
  // never enable CRM-03 auto-creation. allowCreation stays false.
  const allowCreation = false
  const personResult = await resolveOrCreateInboundPerson(
    normalizedEvent,
    { allowCreation, role: 'buyer' },
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
