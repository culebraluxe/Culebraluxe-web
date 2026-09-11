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
        description: 'Return canonical Person-to-Property relationships.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.forPerson(request.personId),
      },
      [PROPERTY_OPERATIONS.UPSERT_FOR_PERSON]: {
        kind: 'command',
        description: 'Create or enrich a canonical Property and attach its contextual relationship to a Person.',
        authorization: 'property.write',
        execution: { mode: 'ordered', partitionBy: 'personId' },
        handle: async (request, context) => {
          const linked = await this.repository.upsertForPerson(request)
          await this.emit(
            {
              type: 'property.person_context_upserted',
              aggregateId: linked.property.id,
              payload: {
                personId: request.personId,
                propertyId: linked.property.id,
                relation: linked.relation,
                sourceType: request.sourceType ?? 'manual',
              },
            },
            context,
          )
          return linked
        },
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
      // ---- Public inventory reads -------------------------------------------------
      // Queries, so the kernel's default lets an unauthenticated (GUEST) public page
      // read them while still denying every command. Out-of-band authorization is
      // deliberately absent: the public site has no principal, and an ACTIVE LISTING
      // is public information. Internal/private Property detail stays behind
      // property.get, which is never exposed to a public surface.
      [PROPERTY_OPERATIONS.LIST]: {
        kind: 'query',
        description: 'List inventory. publicOnly=true returns ACTIVE LISTINGS only (public surfaces).',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.list(request),
      },
      [PROPERTY_OPERATIONS.SEARCH]: {
        kind: 'query',
        description: 'Structured buyers search over active-listing inventory, plus the stable view vocabulary.',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.search(request),
      },
      [PROPERTY_OPERATIONS.SIMILAR]: {
        kind: 'query',
        description: 'Deterministic similar-listing suggestions from canonical Property fields.',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.similar(request),
      },
      [PROPERTY_OPERATIONS.BY_SLUG]: {
        kind: 'query',
        description: 'Load the full public listing payload for one slug.',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.bySlug(request),
      },
      [PROPERTY_OPERATIONS.PUBLIC_SLUGS]: {
        kind: 'query',
        description: 'The set of currently live public listing slugs (stale recently-viewed entries drop out).',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async () => this.repository.publicSlugs(),
      },
      [PROPERTY_OPERATIONS.INTRO]: {
        kind: 'query',
        description: 'Minimal public Property reference (id, name, location) for intros and cross-links.',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.intro(request),
      },
    }
  }

  invariants() {
    return [
      'Property is the canonical home for address/place truth; Person does not own address fields.',
      'Person-to-Property relationships supply context such as legal_address, physical_property, address, or interest.',
      'localName and legalOwnerName are optional Property qualifiers, not separate entities and never surrogate Persons.',
      'External address observations such as Apple Contacts remain provenance until matched or promoted to canonical Property.',
      'Forms may hydrate from and enrich Property; issued Contracts snapshot the exact Property facts they used.',
      'Contract-specific terms do not belong in Property.',
      'Property persistence is reachable only through the Property repository boundary.',
    ] as const
  }
}
