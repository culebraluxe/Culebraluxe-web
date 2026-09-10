// ---------------------------------------------------------------------------
// TESTV2 — Project service (parent of WBS work items). Shelf domain, DB-free.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { ProjectService } from '../services/project'
import type { CreateProjectRequest, Project, ProjectRepository } from '../services/project'
import { capturingInfrastructure, context } from './test-support'

const actor = { id: 'u-1', kind: 'user' as const }

class MemoryProjectRepository implements ProjectRepository {
  private readonly map = new Map<string, Project>()
  seed(p: Project): this {
    this.map.set(p.id, p)
    return this
  }
  async get(id: string) {
    return this.map.get(id) ?? null
  }
  async list() {
    return [...this.map.values()]
  }
  async create(request: CreateProjectRequest): Promise<Project> {
    const p: Project = {
      id: request.id,
      name: request.name,
      owner: null,
      status: 'open',
      description: request.description ?? '',
      areas: (request.areas as Project['areas']) ?? [],
      projectType: null, playbookId: null, playbookVersion: null, personId: null, propertyId: null, contractId: null,
      startsAt: null,
      endsAt: null,
      createdAt: '2026-01-01T00:00:00.000Z',
      updatedAt: '2026-01-01T00:00:00.000Z',
    }
    Object.assign(p, {
      projectType: request.projectType ?? null, playbookId: request.playbookId ?? null,
      playbookVersion: request.playbookVersion ?? null, personId: request.personId ?? null,
      propertyId: request.propertyId ?? null, contractId: request.contractId ?? null,
    })
    this.map.set(p.id, p)
    return p
  }
  async update(request: { id: string; name?: string; status?: string }): Promise<Project> {
    const existing = this.map.get(request.id)
    if (!existing) throw new Error(`Project not found: ${request.id}`)
    const updated = { ...existing, name: request.name ?? existing.name, status: (request.status as Project['status']) ?? existing.status, updatedAt: '2026-02-01T00:00:00.000Z' }
    this.map.set(request.id, updated)
    return updated
  }
  async complete(request: { id: string }): Promise<Project> {
    const existing = this.map.get(request.id)
    if (!existing) throw new Error(`Project not found: ${request.id}`)
    const updated = { ...existing, status: 'done' as const, updatedAt: '2026-02-02T00:00:00.000Z' }
    this.map.set(request.id, updated)
    return updated
  }
}

test('project.create persists a parent project and emits project.created', async () => {
  const repo = new MemoryProjectRepository()
  const infra = capturingInfrastructure()
  const service = new ProjectService(repo, infra.infrastructure)
  assert.deepEqual(service.describe().dependencies, ['wbs'])
  const res = await service.execute({
    operation: 'project.create',
    payload: { id: 'p1', name: 'Onboard Ana', description: 'Casa Luar onboarding', areas: ['clients', 'properties'] },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.status, 'open')
    assert.deepEqual(res.value.areas, ['clients', 'properties'])
  }
  assert.ok(infra.events.some((e) => e.type === 'project.created' && e.aggregateId === 'p1'))
})

test('project.create preserves playbook identity and context anchors', async () => {
  const repo = new MemoryProjectRepository()
  const service = new ProjectService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'project.create',
    payload: { id: 'p-playbook', name: 'Synthetic Listing', projectType: 'listing', playbookId: 'listing-onboarding', playbookVersion: 1, personId: 'person-test', propertyId: 'property-test' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.projectType, 'listing')
    assert.equal(res.value.playbookId, 'listing-onboarding')
    assert.equal(res.value.playbookVersion, 1)
    assert.equal(res.value.propertyId, 'property-test')
  }
})

test('project.instantiate is replay-safe for the same playbook', async () => {
  const repo = new MemoryProjectRepository()
  const createdItems = [{ id: 'p-replay-parties', projectId: 'p-replay' }] as any
  const infra = capturingInfrastructure()
  const router = { dispatch: async () => ({ ok: true, value: createdItems, correlationId: 'c' }) } as any
  const service = new ProjectService(repo, { ...infra.infrastructure, router })
  const request = { id: 'p-replay', name: 'Replay Listing', playbookId: 'listing-onboarding', playbookVersion: 1 }
  const first = await service.execute({ operation: 'project.instantiate', payload: request, context: context({ actor }) })
  assert.equal(first.ok, true)
  const second = await service.execute({ operation: 'project.instantiate', payload: request, context: context({ actor }) })
  assert.equal(second.ok, true)
  if (second.ok) assert.deepEqual(second.value.itemIds, ['p-replay-parties'])
})

test('project.instantiate rejects an unknown or conflicting playbook', async () => {
  const repo = new MemoryProjectRepository()
  const infra = capturingInfrastructure()
  const router = { dispatch: async () => ({ ok: true, value: [], correlationId: 'c' }) } as any
  const service = new ProjectService(repo, { ...infra.infrastructure, router })
  const unknown = await service.execute({
    operation: 'project.instantiate',
    payload: { id: 'p-unknown', name: 'Unknown', playbookId: 'missing', playbookVersion: 1 },
    context: context({ actor }),
  })
  assert.equal(unknown.ok, false)
  if (!unknown.ok) assert.equal(unknown.error.code, 'PROJECT_PLAYBOOK_NOT_FOUND')

  await service.execute({
    operation: 'project.create',
    payload: { id: 'p-conflict', name: 'Legacy project' },
    context: context({ actor }),
  })
  const conflict = await service.execute({
    operation: 'project.instantiate',
    payload: { id: 'p-conflict', name: 'Listing', playbookId: 'listing-onboarding', playbookVersion: 1 },
    context: context({ actor }),
  })
  assert.equal(conflict.ok, false)
  if (!conflict.ok) assert.equal(conflict.error.code, 'PROJECT_PLAYBOOK_CONFLICT')
})

test('project.instantiate validates custom nodes before creating the parent', async () => {
  const repo = new MemoryProjectRepository()
  const service = new ProjectService(repo, { ...capturingInfrastructure().infrastructure, router: { dispatch: async () => ({ ok: true, value: {}, correlationId: 'c' }) } as any })
  const result = await service.execute({
    operation: 'project.instantiate',
    payload: {
      id: 'p-invalid', name: 'Invalid', playbookId: 'listing-onboarding', playbookVersion: 1,
      nodes: [{ key: 'child', title: 'Child', category: 'clients', parentKey: 'missing', order: 1 }],
    },
    context: context({ actor }),
  })
  assert.equal(result.ok, false)
  if (!result.ok) assert.equal(result.error.code, 'PROJECT_PLAYBOOK_NODE_INVALID')
  assert.equal(await repo.get('p-invalid'), null)
})

test('project.get/list/complete round-trip through the service', async () => {
  const repo = new MemoryProjectRepository()
  const service = new ProjectService(repo, capturingInfrastructure().infrastructure)
  await service.execute({ operation: 'project.create', payload: { id: 'p1', name: 'Onboard Ana' }, context: context({ actor }) })
  const got = await service.execute({ operation: 'project.get', payload: { id: 'p1' }, context: context({ actor }) })
  assert.equal(got.ok, true)
  if (got.ok) assert.equal(got.value?.name, 'Onboard Ana')
  const done = await service.execute({ operation: 'project.complete', payload: { id: 'p1' }, context: context({ actor }) })
  assert.equal(done.ok, true)
  if (done.ok) assert.equal(done.value.status, 'done')
  const list = await service.execute({ operation: 'project.list', payload: {}, context: context({ actor }) })
  assert.equal(list.ok, true)
  if (list.ok) assert.equal(list.value.length, 1)
})

test('project.update validates names, areas, and playbook versions before persistence', async () => {
  const repo = new MemoryProjectRepository().seed({
    id: 'p-validation', name: 'Valid', owner: null, status: 'open', description: '', areas: [],
    projectType: null, playbookId: null, playbookVersion: null, personId: null, propertyId: null, contractId: null,
    startsAt: null, endsAt: null, createdAt: null, updatedAt: null,
  })
  const service = new ProjectService(repo, capturingInfrastructure().infrastructure)
  for (const payload of [
    { name: '' },
    { areas: ['not-a-category'] as any },
    { playbookVersion: 0 },
  ]) {
    const result = await service.execute({ operation: 'project.update', payload: { id: 'p-validation', ...payload }, context: context({ actor }) })
    assert.equal(result.ok, false)
  }
  assert.equal((await repo.get('p-validation'))?.name, 'Valid')
})
