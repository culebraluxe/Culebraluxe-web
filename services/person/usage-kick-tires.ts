import type { ServiceContext } from '../core'
import type { PersonEnvelopeService } from './person-service-envelope'
import type { PersonMethodService } from './person-service-methods'
import { PERSON_OPERATIONS, type PersonIdentityDto } from './types'

/** Side-by-side caller shapes for the architecture experiment. */
export function getPersonViaEnvelope(
  service: PersonEnvelopeService,
  context: ServiceContext,
  personId: string,
) {
  return service.execute({
    operation: PERSON_OPERATIONS.GET.name,
    payload: { personId },
    context,
  })
}

export function getPersonViaMethod(
  service: PersonMethodService,
  context: ServiceContext,
  personId: string,
) {
  return service.get({ personId }, context)
}

export function attachIdentityViaEnvelope(
  service: PersonEnvelopeService,
  context: ServiceContext,
  personId: string,
  identity: PersonIdentityDto,
) {
  return service.execute({
    operation: PERSON_OPERATIONS.ATTACH_IDENTITY.name,
    payload: { personId, identity },
    context,
  })
}

export function attachIdentityViaMethod(
  service: PersonMethodService,
  context: ServiceContext,
  personId: string,
  identity: PersonIdentityDto,
) {
  return service.attachIdentity({ personId, identity }, context)
}
