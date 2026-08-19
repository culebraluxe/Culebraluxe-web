import { normalizeInboundEvent } from './crm-intake-normalization'
import { prepareInboundEvent } from './crm-intake'
import { resolveOrCreateInboundPerson } from './crm-person-creation'
import type { IntakeRepositories } from './crm-intake-types'
import type { PersonCreationRepositories } from './crm-person-types'
import { adaptCommunicationsEvent } from './crm-communications-normalization'
import type {
  CommunicationsAdapterConfiguration,
  CommunicationsIntakeResult,
  CommunicationsProviderEvent,
} from './crm-communications-types'

export interface CommunicationsIntakeRepositories
  extends IntakeRepositories,
    PersonCreationRepositories {}

export async function prepareCommunicationsIntake(
  event: CommunicationsProviderEvent,
  configuration: CommunicationsAdapterConfiguration,
  repositories: CommunicationsIntakeRepositories,
): Promise<CommunicationsIntakeResult> {
  const adapted = adaptCommunicationsEvent(event, configuration)
  if (adapted.status !== 'accepted') return adapted

  const normalizedEvent = normalizeInboundEvent(adapted.inboundEvent)
  const duplicate = await repositories.findInteractionBySourceIdentity(
    normalizedEvent.source.system,
    normalizedEvent.source.externalId,
  )
  if (duplicate) return { status: 'duplicate', existingInteractionId: duplicate.id }

  const allowCreation =
    adapted.direction === 'inbound' &&
    normalizedEvent.actor.identityHints[0]?.evidence !== 'user_supplied' &&
    Boolean(adapted.applicableCreationRole)
  const personResult = await resolveOrCreateInboundPerson(
    normalizedEvent,
    { allowCreation, role: adapted.applicableCreationRole ?? 'buyer' },
    repositories,
  )
  if (personResult.status === 'duplicate' && personResult.existingInteractionId) {
    return { status: 'duplicate', existingInteractionId: personResult.existingInteractionId }
  }
  if (personResult.status !== 'created' && personResult.status !== 'resolved_existing') {
    return {
      status: personResult.status === 'rejected' || personResult.status === 'conflicting'
        ? 'rejected'
        : 'resolution_required',
      reason: personResult.reason ?? personResult.status,
      personResult,
    }
  }

  const intakeResult = await prepareInboundEvent(
    {
      ...adapted.inboundEvent,
      actor: { ...adapted.inboundEvent.actor, personId: personResult.personId },
    },
    repositories,
  )
  if (intakeResult.status === 'duplicate' && intakeResult.existingInteractionId) {
    return { status: 'duplicate', existingInteractionId: intakeResult.existingInteractionId }
  }
  if (intakeResult.status !== 'ready') {
    return {
      status: intakeResult.status === 'resolution_required' ? 'resolution_required' : 'rejected',
      reason: `crm_intake_${intakeResult.status}`,
      personResult,
    }
  }
  return { status: 'ready', personResult, intakeResult }
}
