import { test } from 'node:test'
import assert from 'node:assert/strict'
import { FirmService } from '../services/firm'
import type { FirmDto, FirmRepository } from '../services/firm'
import { capturingInfrastructure, context } from './test-support'
import { EntitlementService } from '../services/entitlement'

// ---------------------------------------------------------------------------
// TESTV2 — Firm service envelope tests.
// ---------------------------------------------------------------------------

class MemoryFirmRepository implements FirmRepository {
  private readonly map = new Map<string, FirmDto>()
  private seq = 0
  seed(dto: FirmDto): this {
    this.map.set(dto.id, dto)
    return this
  }
  async get(firmId: string): Promise<FirmDto | null> {
    return this.map.get(firmId) ?? null
  }
  async findByName(request: { name: string }): Promise<FirmDto | null> {
    for (const f of this.map.values()) if (f.name === request.name) return f
    return null
  }
  async upsert(request: {
    firmId?: string
    name: string
    legalName?: string | null
    kind?: string | null
    status?: string
  }): Promise<FirmDto> {
    const id = request.firmId ?? `firm-${++this.seq}`
    const existing = this.map.get(id)
    const dto: FirmDto = {
      id,
      name: request.name,
      legalName: request.legalName ?? existing?.legalName ?? null,
      kind: request.kind ?? existing?.kind ?? null,
      status: request.status ?? existing?.status ?? 'active',
    }
    this.map.set(id, dto)
    return dto
  }
}

const actor = { id: 'u-1', kind: 'user' as const }

test('firm.get returns the firm DTO for a known id', async () => {
  const repo = new MemoryFirmRepository().seed({ id: 'f1', name: 'Culebra', legalName: null, kind: 'brokerage', status: 'active' })
  const service = new FirmService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'firm.get', payload: { firmId: 'f1' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value?.name, 'Culebra')
})

test('firm.upsert creates when no id is supplied and updates an existing firm', async () => {
  const repo = new MemoryFirmRepository()
  const infra = capturingInfrastructure()
  const service = new FirmService(repo, infra.infrastructure)
  const created = await service.execute({ operation: 'firm.upsert', payload: { name: 'Sea Holdings' }, context: context({ actor }) })
  assert.equal(created.ok, true)
  if (!created.ok) return
  const id = created.value.id
  const updated = await service.execute({
    operation: 'firm.upsert',
    payload: { firmId: id, name: 'Sea Holdings', legalName: 'Sea Holdings LLC' },
    context: context({ actor }),
  })
  assert.equal(updated.ok, true)
  if (updated.ok) assert.equal(updated.value.legalName, 'Sea Holdings LLC')
})

test('firm.findByName resolves an upserted firm by exact name', async () => {
  const repo = new MemoryFirmRepository()
  const service = new FirmService(repo, capturingInfrastructure().infrastructure)
  await service.execute({ operation: 'firm.upsert', payload: { name: 'Reef Realty' }, context: context({ actor }) })
  const found = await service.execute({ operation: 'firm.findByName', payload: { name: 'Reef Realty' }, context: context({ actor }) })
  assert.equal(found.ok, true)
  if (found.ok) assert.equal(found.value?.name, 'Reef Realty')
})

test('firm.get for an unknown id is ok:true, value:null', async () => {
  const service = new FirmService(new MemoryFirmRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'firm.get', payload: { firmId: 'nope' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value, null)
})


test('firm.upsert rejects an empty name with FIRM_NAME_REQUIRED', async () => {
  const repo = new MemoryFirmRepository()
  const service = new FirmService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'firm.upsert', payload: { name: '   ' }, context: context({ actor }) })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FIRM_NAME_REQUIRED')
})

test('firm.upsert with an explicit firmId still works and stays keyed by firm', async () => {
  const repo = new MemoryFirmRepository()
  const service = new FirmService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'firm.upsert',
    payload: { firmId: 'fx-1', name: 'Anchor Brokerage' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.id, 'fx-1')
})

test('an unknown operation on firm returns UNKNOWN_OPERATION', async () => {
  const service = new FirmService(new MemoryFirmRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'firm.nope', payload: {}, context: context({ actor }) } as never)
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'UNKNOWN_OPERATION')
})

test('GUEST cannot run a firm command (FORBIDDEN under the entitlement stub)', async () => {
  const repo = new MemoryFirmRepository()
  const service = new FirmService(repo, { authorization: new EntitlementService() })
  const res = await service.execute({
    operation: 'firm.upsert',
    payload: { name: 'Coastal LLC' },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FORBIDDEN')
})
