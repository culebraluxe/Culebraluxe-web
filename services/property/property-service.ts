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
  readonly description = 'Owns canonical address/place identity, structured address, and Property qualifiers.'
  protected readonly operations: ServiceOperationDefinitions<PropertyOperationMap>

  constructor(
    private readonly repository: PropertyRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [PROPERTY_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical Property by id, including reusable structured address and qualifiers.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.propertyId),
      },
      [PROPERTY_OPERATIONS.FIND_BY_ADDRESS]: {
        kind: 'query',
        description: 'Resolve a canonical Property by normalized structured-address input.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.findByAddress(request),
      },
      [PROPERTY_OPERATIONS.FOR_PERSON]: {
        kind: 'query',
        description: 'Return canonical Person-to-Property relationships plus unpromoted address evidence.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.forPerson(request.personId),
      },
      [PROPERTY_OPERATIONS.SET_DISPLAY_NAME]: {
        kind: 'command',
        description: 'Compatibility command for changing the optional local/property name.',
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
        description: 'Change canonical Property status.',
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
      'Property is the canonical home for address/place truth; Person does not own address fields.',
      'Person-to-Property relationships supply context such as legal_address, physical_property, address, or interest.',
      'localName and legalOwnerName are optional Property qualifiers, not separate entities and never surrogate Persons.',
      'External address observations such as Apple Contacts remain provenance until matched or promoted to canonical Property.',
      'Contract-specific terms do not belong in Property; issued Contracts snapshot the Property facts they used.',
      'Property persistence is reachable only through the Property repository boundary.',
    ] as const
  }
}
