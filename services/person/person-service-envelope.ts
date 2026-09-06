import {
  BaseService,
  type ServiceContext,
  type ServiceInfrastructure,
  type ServiceResult,
} from '../core'
import type { PersonRepository } from './repository'
import {
  PERSON_OPERATIONS,
  type AttachPersonIdentityRequest,
  type FindPersonByIdentityRequest,
  type GetPersonRequest,
  type PersonEnvelope,
  type PersonOperationName,
  type PersonRequestByOperation,
  type PersonResponse,
  type PersonResponseByOperation,
  type SetPersonDisplayNameRequest,
} from './types'

type EnvelopeHandler = (
  payload: PersonRequestByOperation[PersonOperationName],
  context: ServiceContext,
) => Promise<PersonResponseByOperation[PersonOperationName]>

/**
 * Kick-the-tires version A: one public verb, operation catalog + DTO envelopes.
 * New capabilities are registered as operations rather than new public methods.
 */
export class PersonEnvelopeService extends BaseService {
  readonly domain = 'person'
  readonly version = '0-envelope'
  readonly description = 'Canonical Person service — envelope/operation-map experiment.'

  private readonly handlers: ReadonlyMap<PersonOperationName, EnvelopeHandler>

  constructor(
    private readonly repository: PersonRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.handlers = new Map<PersonOperationName, EnvelopeHandler>([
      [PERSON_OPERATIONS.GET.name, (payload) => this.get(payload as GetPersonRequest)],
      [
        PERSON_OPERATIONS.FIND_BY_IDENTITY.name,
        (payload) => this.findByIdentity(payload as FindPersonByIdentityRequest),
      ],
      [
        PERSON_OPERATIONS.SET_DISPLAY_NAME.name,
        (payload, context) => this.setDisplayName(payload as SetPersonDisplayNameRequest, context),
      ],
      [
        PERSON_OPERATIONS.ATTACH_IDENTITY.name,
        (payload, context) => this.attachIdentity(payload as AttachPersonIdentityRequest, context),
      ],
    ])
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

  async execute<K extends PersonOperationName>(
    envelope: PersonEnvelope<K>,
  ): Promise<ServiceResult<PersonResponse<K>>> {
    return this.run<PersonResponse<K>>(envelope.operation, envelope.context, async () => {
      const capability = this.capabilities().find((candidate) => candidate.name === envelope.operation)
      if (!capability) this.fail('UNKNOWN_OPERATION', `Unknown Person operation: ${envelope.operation}`)

      await this.authorize(capability.authorization ?? envelope.operation, envelope.context)

      const handler = this.handlers.get(envelope.operation)
      if (!handler) this.fail('UNKNOWN_OPERATION', `No Person handler registered for: ${envelope.operation}`)

      return (await handler(
        envelope.payload as PersonRequestByOperation[PersonOperationName],
        envelope.context,
      )) as PersonResponse<K>
    })
  }

  private get(request: GetPersonRequest) {
    return this.repository.get(request.personId)
  }

  private findByIdentity(request: FindPersonByIdentityRequest) {
    return this.repository.findByIdentity(request)
  }

  private async setDisplayName(request: SetPersonDisplayNameRequest, context: ServiceContext) {
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
  }

  private async attachIdentity(request: AttachPersonIdentityRequest, context: ServiceContext) {
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
  }
}
