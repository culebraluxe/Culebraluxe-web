import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { PropertyRepository } from './repository'
import {
  PROPERTY_OPERATIONS,
  type PropertyOperationMap,
} from './types'

/** Canonical Property service using the same envelope + queued BaseService runtime. */
export class PropertyService extends BaseService<PropertyOperationMap> {
  readonly domain = 'property'
  readonly version = '1'
  readonly description = 'Owns canonical property identity, structured address, and property state.'
  protected readonly operations: ServiceOperationDefinitions<PropertyOperationMap>

  constructor(
    private readonly repository: PropertyRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [PROPERTY_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical property by id, including reusable structured address.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.propertyId),
      },
      [PROPERTY_OPERATIONS.FIND_BY_ADDRESS]: {
        kind: 'query',
        description: 'Resolve a canonical property by normalized structured-address input.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.findByAddress(request),
      },
      [PROPERTY_OPERATIONS.FOR_PERSON]: {
        kind: 'query',
        description: 'Return canonical Property relationships plus observed address candidates for one Person.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.forPerson(request.personId),
      },
      [PROPERTY_OPERATIONS.SET_DISPLAY_NAME]: {
        kind: 'command',
        description: 'Change the canonical display name for a property.',
        authorization: 'property.write',
        execution: { mode: 'ordered', partitionBy: 'propertyId' },
        handle: async (request, context) => {
          const property = await this.repository.setDisplayName(request)
          await this.emit(
            {
              type: 'property.display_name_changed',
              aggregateId: property.id,
              payload: { propertyId: property.id, displayName: property.displayName },
            },
            context,
          )
          return property
        },
      },
      [PROPERTY_OPERATIONS.SET_STATUS]: {
        kind: 'command',
        description: 'Change canonical property status.',
        authorization: 'property.write',
        execution: { mode: 'ordered', partitionBy: 'propertyId' },
        handle: async (request, context) => {
          const property = await this.repository.setStatus(request)
          await this.emit(
            {
              type: 'property.status_changed',
              aggregateId: property.id,
              payload: { propertyId: property.id, status: property.status },
            },
            context,
          )
          return property
        },
      },
    }
  }

  invariants() {
    return [
      'Property owns canonical property identity, structured physical address, and property facts independent of any Contract.',
      'External address observations such as Apple Contacts remain evidence until matched or promoted to canonical Property.',
      'Person may reference Property context, but Person does not own canonical property-address truth.',
      'Listing, buyer, seller, and deal-specific semantics do not belong in Property.',
      'Property persistence is reachable only through the Property repository boundary.',
    ] as const
  }
}
