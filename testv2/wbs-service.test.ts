// ---------------------------------------------------------------------------
// TESTV2 — WBS service (Work Items + Projects). Mirrors the kernel test style.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { WbsService } from '../services/wbs'
import type { WbsRepository } from '../services/wbs'
import { isWbsCategory, MANAGEMENT_CATEGORY_ID, WBS_CATEGORIES } from '../services/wbs'
import type { WbsItem, WbsProject } from '../services/wbs'
import { capturingInfrastructure, context } from './test-support'

const actor = { id: 'u-1', kind: 'user' as const }

function baseItem(id: string, over: Partial<WbsItem> = {}): WbsItem {
  return {
    id,
    title: `Item ${id}`,
    notes: '',
    category: 'management',
    status: 'open',
    projectId: null,
    parentId: null,
    dueAt: null,
    owner: null,
    order: null,
    entity: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    ...over,
  }
}

class MemoryWbsRepository implements WbsRepository {
  private readonly items = new Map<string, WbsItem>()
  private readonly projects = new Map<string, WbsProject>()
  seed(item: WbsItem): this {
    this.items.set(item.id, item)
    return this
  }
  async get(id: string) {
    return this.items.get(id) ?? null
  }
  async listDue(request: { category?: string }) {
    return [...this.items.values()]
      .filter((i) => i.status === 'open' || i.status === 'doing')
      .filter((i) => !request.category || i.category === request.category)
      .sort((a, b) => (a.dueAt ?? '9999').localeCompare(b.dueAt ?? '9999'))
  }
  async create(request: { id: string; title: string; category: string; projectId?: string | null }): Promise<WbsItem> {
    const item = baseItem(request.id, {
      title: request.title,
      category: request.category as WbsItem['category'],
      projectId: request.projectId ?? null,
    })
    this.items.set(item.id, item)
    return item
  }
  async save(request: { id: string; title: string; category: string; status?: string }): Promise<WbsItem> {
    const existing = this.items.get(request.id)
    if (!existing) throw new Error(`WBS item not found: ${request.id}`)
    const updated = {
      ...existing,
      title: request.title,
      category: request.category as WbsItem['category'],
      status: (request.status as WbsItem['status']) ?? existing.status,
      updatedAt: '2026-02-01T00:00:00.000Z',
    }
    this.items.set(request.id, updated)
    return updated
  }
  async complete(request: { id: string }): Promise<WbsItem> {
    const existing = this.items.get(request.id)
    if (!existing) throw new Error(`WBS item not found: ${request.id}`)
    const updated = { ...existing, status: 'done' as const, updatedAt: '2026-02-02T00:00:00.000Z' }
    this.items.set(request.id, updated)
    return updated
  }
  async dismiss(request: { id: string }): Promise<WbsItem> {
    const existing = this.items.get(request.id)
    if (!existing) throw new Error(`WBS item not found: ${request.id}`)
    const updated = { ...existing, status: 'dismissed' as const, updatedAt: '2026-02-03T00:00:00.000Z' }
    this.items.set(request.id, updated)
    return updated
  }
  async getProject(id: string): Promise<WbsProject | null> {
    return this.projects.get(id) ?? null
  }
  async listProjects(): Promise<WbsProject[]> {
    return [...this.projects.values()]
  }
  async createProject(request: { id: string; name: string }): Promise<WbsProject> {
    const project: WbsProject = {
      id: request.id,
      name: request.name,
      owner: null,
      status: 'open',
      createdAt: '2026-01-01T00:00:00.000Z',
      updatedAt: '2026-01-01T00:00:00.000Z',
    }
    this.projects.set(request.id, project)
    return project
  }
}

test('the category catalog is plural where it should be and has a Management catch-all', () => {
  assert.deepEqual(WBS_CATEGORIES.map((c) => c.id), [
    'clients',
    'contracts',
    'properties',
    'media',
    'marketing',
    'accounting',
    'management',
  ])
  assert.ok(isWbsCategory('properties'), 'Properties must be a category (plural)')
  assert.ok(isWbsCategory(MANAGEMENT_CATEGORY_ID), 'Management is the catch-all bucket')
  assert.ok(!isWbsCategory('bogus'))
})

test('wbs.create persists a follow-up and emits wbs.created', async () => {
  const repo = new MemoryWbsRepository()
  const infra = capturingInfrastructure()
  const service = new WbsService(repo, infra.infrastructure)
  const res = await service.execute({
    operation: 'wbs.create',
    payload: { id: 'w1', title: 'Take property photos', category: 'media', projectId: 'prj-1' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.status, 'open')
    assert.equal(res.value.category, 'media')
    assert.equal(res.value.projectId, 'prj-1')
  }
  assert.ok(infra.events.some((e) => e.type === 'wbs.created' && e.aggregateId === 'w1'))
})

test('wbs.create rejects an unknown category with WBS_CATEGORY_UNKNOWN', async () => {
  const service = new WbsService(new MemoryWbsRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'wbs.create',
    payload: { id: 'w2', title: 'x', category: 'not-a-bucket' },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'WBS_CATEGORY_UNKNOWN')
})

test('wbs.create rejects a missing title with WBS_TITLE_REQUIRED', async () => {
  const service = new WbsService(new MemoryWbsRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'wbs.create',
    payload: { id: 'w3', title: '   ', category: 'clients' },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'WBS_TITLE_REQUIRED')
})

test('wbs.complete transitions an item to done and emits wbs.completed', async () => {
  const repo = new MemoryWbsRepository().seed(baseItem('w1'))
  const infra = capturingInfrastructure()
  const service = new WbsService(repo, infra.infrastructure)
  const res = await service.execute({ operation: 'wbs.complete', payload: { id: 'w1' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.status, 'done')
  assert.ok(infra.events.some((e) => e.type === 'wbs.completed'))
})

test('wbs.listDue returns only open/doing items, filterable by category', async () => {
  const repo = new MemoryWbsRepository()
    .seed(baseItem('open-client', { category: 'clients', status: 'open' }))
    .seed(baseItem('open-media', { category: 'media', status: 'open' }))
    .seed(baseItem('done-client', { category: 'clients', status: 'done' }))
  const service = new WbsService(repo, capturingInfrastructure().infrastructure)

  const all = await service.execute({ operation: 'wbs.listDue', payload: {}, context: context({ actor }) })
  assert.equal(all.ok, true)
  if (all.ok) {
    assert.equal(all.value.length, 2)
    assert.ok(all.value.every((i) => i.status === 'open' || i.status === 'doing'))
  }

  const clients = await service.execute({
    operation: 'wbs.listDue',
    payload: { category: 'clients' },
    context: context({ actor }),
  })
  assert.equal(clients.ok, true)
  if (clients.ok) assert.deepEqual(clients.value.map((i) => i.id), ['open-client'])
})

test('project.create creates a lightweight project root', async () => {
  const repo = new MemoryWbsRepository()
  const infra = capturingInfrastructure()
  const service = new WbsService(repo, infra.infrastructure)
  const res = await service.execute({
    operation: 'project.create',
    payload: { id: 'prj-1', name: 'Onboard Ana' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.name, 'Onboard Ana')
  assert.ok(infra.events.some((e) => e.type === 'project.created'))
})

test('wbs.dismiss sets an item aside (dismissed) and emits wbs.dismissed', async () => {
  const repo = new MemoryWbsRepository().seed(baseItem('w1'))
  const infra = capturingInfrastructure()
  const service = new WbsService(repo, infra.infrastructure)
  const res = await service.execute({ operation: 'wbs.dismiss', payload: { id: 'w1' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.status, 'dismissed')
  assert.ok(infra.events.some((e) => e.type === 'wbs.dismissed'))
})

test('project.get and project.list return created projects', async () => {
  const repo = new MemoryWbsRepository()
  const service = new WbsService(repo, capturingInfrastructure().infrastructure)
  await service.execute({
    operation: 'project.create',
    payload: { id: 'prj-1', name: 'Onboard Ana' },
    context: context({ actor }),
  })
  await service.execute({
    operation: 'project.create',
    payload: { id: 'prj-2', name: 'List Casa 3' },
    context: context({ actor }),
  })

  const got = await service.execute({ operation: 'project.get', payload: { id: 'prj-1' }, context: context({ actor }) })
  assert.equal(got.ok, true)
  if (got.ok) assert.equal(got.value?.name, 'Onboard Ana')

  const list = await service.execute({ operation: 'project.list', payload: {}, context: context({ actor }) })
  assert.equal(list.ok, true)
  if (list.ok) assert.equal(list.value.length, 2)
})
