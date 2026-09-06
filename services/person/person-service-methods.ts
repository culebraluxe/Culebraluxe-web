import {
  BaseService,
  type ServiceContext,
  type ServiceInfrastructure,
} from '../core'
import type { PersonRepository } from './repository'
import {
  PERSON_OPERATIONS,
  type AttachPersonIdentityRequest,
  type FindPersonByIdentityRequest,
  type GetPersonRequest,
  type SetPersonDisplayNameRequest,
} from './types'

/**
 * Kick-the-tires version B: conventional public methods over the same DTOs and repository.
 * Deliberately kept direct so we can feel the API growth/boilerplate trade-off honestly.
 */
export class PersonMethodService extends BaseService {
  readonly domain = 'person'
  readonly version = '0-methods'
  readonly description = 'Canonical Person service — conventional typed-method experiment.'

  constructor(
    private readonly repository: PersonRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)
  }

  capabilities() {
    return Object.values(PERSON_OPERATIONS)
  }

  invariants() {
    return [
      'Person owns canonical identity; buyer/seller/agent roles are contextual to Contracts.',
      'Person persistence is reachable only through the Person service repository boundary.',
      'Identity attachment must not silently transfer an identity already owned by another Person.',
    ] as const
  }

  async get(request: GetPersonRequest, context: ServiceContext) {
    return this.run(PERSON_OPERATIONS.GET.name, context, async () => {
      await this.authorize(PERSON_OPERATIONS.GET.authorization, context)
      return this.repository.get(request.personId)
    })
  }

  async findByIdentity(request: FindPersonByIdentityRequest, context: ServiceContext) {
    return this.run(PERSON_OPERATIONS.FIND_BY_IDENTITY.name, context, async () => {
      await this.authorize(PERSON_OPERATIONS.FIND_BY_IDENTITY.authorization, context)
      return this.repository.findByIdentity(request)
    })
  }

  async setDisplayName(request: SetPersonDisplayNameRequest, context: ServiceContext) {
    return this.run(PERSON_OPERATIONS.SET_DISPLAY_NAME.name, context, async () => {
      await this.authorize(PERSON_OPERATIONS.SET_DISPLAY_NAME.authorization, context)
      const person = await this.repository.setDisplayName(request)
      await this.emit(
        {
          type: 'person.display_name_changed',
          aggregateId: person.id,
          payload: { personId: person.id, displayName: person.displayName },
        },
        context,
      )
      return person
    })
  }

  async attachIdentity(request: AttachPersonIdentityRequest, context: ServiceContext) {
    return this.run(PERSON_OPERATIONS.ATTACH_IDENTITY.name, context, async () => {
      await this.authorize(PERSON_OPERATIONS.ATTACH_IDENTITY.authorization, context)
      const identity = await this.repository.attachIdentity(request)
      await this.emit(
        {
          type: 'person.identity_attached',
          aggregateId: request.personId,
          payload: {
            personId: request.personId,
            kind: identity.kind,
            value: identity.value,
            sourceSystem: identity.sourceSystem,
          },
        },
        context,
      )
      return identity
    })
  }
}
