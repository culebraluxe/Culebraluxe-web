import { test } from 'node:test'
import assert from 'node:assert/strict'
import { PropertyService } from '../services/property'
import type {
  PropertyDto,
  PropertyForPersonDto,
  PropertyRepository,
  UpsertPropertyForPersonRequest,
} from '../services/property'
import { capturingInfrastructure, context } from './test-support'
import { EntitlementService } from '../services/entitlement'

// ---------------------------------------------------------------------------
// TESTV2 — Property service envelope tests (address/place canonical DTO).
// ---------------------------------------------------------------------------

const emptyAddress = {
  addressLine1: null,
  city: null,
  stateOrProvince: null,
  neighborhood: null,
  postalCode: null,
  country: null,
  isoCountryCode: null,
}

function prop(over: Partial<PropertyDto> = {}): PropertyDto {
  return {
    id: 'pr1',
    displayName: 'Casa Luar',
    localName: null,
    legalOwnerName: null,
    addressLine1: null,
    municipality: null,
    address: emptyAddress,
    status: 'active',
    archivedAt: null,
    ...over,
  }
}

class MemoryPropertyRepository implements PropertyRepository {
  private readonly map = new Map<string, PropertyDto>()
  seed(dto: PropertyDto): this {
    this.map.set(dto.id, dto)
    return this
  }
  async get(propertyId: string): Promise<PropertyDto | null> {
    return this.map.get(propertyId) ?? null
  }
  async findByAddress(): Promise<PropertyDto | null> {
    return null
  }
  async forPerson(personId: string) {
    return { personId, properties: [], observedAddresses: [] }
  }
  async upsertForPerson(request: UpsertPropertyForPersonRequest): Promise<PropertyForPersonDto> {
    const property = [...this.map.values()][0] ?? prop()
    return { relation: request.relation, relationStatus: request.relationStatus ?? null, property }
  }
  async setDisplayName(request: { propertyId: string; displayName: string }): Promise<PropertyDto> {
    const existing = this.map.get(request.propertyId)
    if (!existing) throw new Error('not found')
    const updated = { ...existing, displayName: request.displayName }
    this.map.set(request.propertyId, updated)
    return updated
  }
  async setStatus(request: { propertyId: string; status: string }): Promise<PropertyDto> {
    const existing = this.map.get(request.propertyId)
    if (!existing) throw new Error('not found')
    const updated = { ...existing, status: request.status }
    this.map.set(request.propertyId, updated)
    return updated
  }
}

const actor = { id: 'u-1', kind: 'user' as const }

test('property.get returns the canonical PropertyDto', async () => {
  const repo = new MemoryPropertyRepository().seed(prop())
  const service = new PropertyService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'property.get', payload: { propertyId: 'pr1' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value?.displayName, 'Casa Luar')
})

test('property.setDisplayName updates the presentation name', async () => {
  const repo = new MemoryPropertyRepository().seed(prop())
  const service = new PropertyService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'property.setDisplayName', payload: { propertyId: 'pr1', displayName: 'Casa del Mar' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.displayName, 'Casa del Mar')
})

test('property.setStatus transitions the property status', async () => {
  const repo = new MemoryPropertyRepository().seed(prop())
  const service = new PropertyService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'property.setStatus', payload: { propertyId: 'pr1', status: 'archived' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.status, 'archived')
})

test('property.forPerson returns an empty context when nothing is linked', async () => {
  const service = new PropertyService(new MemoryPropertyRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'property.forPerson', payload: { personId: 'p9' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.deepEqual(res.value.properties, [])
})

test('an unknown operation on property returns UNKNOWN_OPERATION', async () => {
  const service = new PropertyService(new MemoryPropertyRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'property.nope', payload: {}, context: context({ actor }) } as never)
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'UNKNOWN_OPERATION')
})

test('GUEST cannot run a property command (FORBIDDEN under the entitlement stub)', async () => {
  const repo = new MemoryPropertyRepository().seed(prop())
  const service = new PropertyService(repo, { authorization: new EntitlementService() })
  const res = await service.execute({
    operation: 'property.setStatus',
    payload: { propertyId: 'pr1', status: 'archived' },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FORBIDDEN')
})
