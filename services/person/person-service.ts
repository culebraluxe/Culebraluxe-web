import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { PersonRepository } from './repository'
import {
  PERSON_OPERATIONS,
  type PersonOperationMap,
} from './types'

/** Canonical Person service: one envelope ingress, typed operation catalog, queued parent runtime. */
export class PersonService extends BaseService<PersonOperationMap> {
  readonly domain = 'person'
  readonly version = '1'
  readonly description = 'Owns canonical people and identity attachment.'
  protected readonly operations: ServiceOperationDefinitions<PersonOperationMap>

  constructor(
    private readonly repository: PersonRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [PERSON_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical person by id.',
        authorization: 'person.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.personId),
      },
      [PERSON_OPERATIONS.FIND_BY_IDENTITY]: {
        kind: 'query',
        description: 'Resolve a canonical person by one identity hint.',
        authorization: 'person.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.findByIdentity(request),
      },
      [PERSON_OPERATIONS.SET_DISPLAY_NAME]: {
        kind: 'command',
        description: 'Change the canonical display name for a person.',
        authorization: 'person.write',
        execution: { mode: 'ordered', partitionBy: 'personId' },
        handle: async (request, context) => {
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
        },
      },
      [PERSON_OPERATIONS.ATTACH_IDENTITY]: {
        kind: 'command',
        description: 'Attach an identity to the canonical person.',
        authorization: 'person.write',
        execution: { mode: 'ordered', partitionBy: 'personId' },
        handle: async (request, context) => {
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
        },
      },
    }
  }

  invariants() {
    return [
      'Person owns canonical identity; buyer/seller/agent roles are contextual to Contracts.',
      'Person persistence is reachable only through the Person repository boundary.',
      'Identity attachment must not silently transfer an identity already owned by another Person.',
    ] as const
  }
}
