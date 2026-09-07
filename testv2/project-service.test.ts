// ---------------------------------------------------------------------------
// TESTV2 — Project service (parent of WBS work items). Shelf domain, DB-free.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { ProjectService } from '../services/project'
import type { Project, ProjectRepository } from '../services/project'
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
  async create(request: { id: string; name: string; description?: string; areas?: readonly string[] }): Promise<Project> {
    const p: Project = {
      id: request.id,
      name: request.name,
      owner: null,
      status: 'open',
      description: request.description ?? '',
      areas: (request.areas as Project['areas']) ?? [],
      startsAt: null,
      endsAt: null,
      createdAt: '2026-01-01T00:00:00.000Z',
      updatedAt: '2026-01-01T00:00:00.000Z',
    }
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
