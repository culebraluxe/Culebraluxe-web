import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { ServiceRegistry } from '../services/core'
import { ProjectService } from '../services/project'
import { WbsService } from '../services/wbs'
import { SqlProjectRepository } from '../db/project-service-repository'
import { SqlWbsRepository } from '../db/wbs-service-repository'
import type { QueryExecutor } from '../db/query-executor'
import { capturingInfrastructure, context } from './test-support'

test('only the canonical Project service advertises Project operations', () => {
  const execute: QueryExecutor = async () => { throw new Error('unexpected DB call') }
  const registry = new ServiceRegistry()
  registry.register(new WbsService(new SqlWbsRepository(execute)))
  registry.register(new ProjectService(new SqlProjectRepository(execute)))
  const owners = registry.list().flatMap(service => service.capabilities
    .filter(capability => capability.name.startsWith('project.'))
    .map(capability => [service.domain, capability.name]))
  assert.deepEqual(owners, [
    ['project', 'project.get'], ['project', 'project.list'],
    ['project', 'project.create'], ['project', 'project.update'],
    ['project', 'project.complete'], ['project', 'project.instantiate'],
  ])
})

for (const operation of ['project.create', 'project.get', 'project.list']) {
  test(`WBS rejects retired ${operation} without persistence or events`, async () => {
    let reads = 0
    const execute: QueryExecutor = async () => { reads++; return [] }
    const infra = capturingInfrastructure()
    const service = new WbsService(new SqlWbsRepository(execute), infra.infrastructure)
    const result = await service.dispatch({
      operation, payload: { id: 'p1', name: 'Not a WBS-owned project' }, context: context(),
    })
    assert.equal(result.ok, false)
    if (!result.ok) assert.equal(result.error.code, 'UNKNOWN_OPERATION')
    assert.equal(reads, 0)
    assert.equal(infra.events.length, 0)
  })
}

test('canonical Project read preserves archived status and normalizes driver dates', async () => {
  let statement = ''
  const execute: QueryExecutor = async strings => {
    statement = strings.join('?')
    return [{
      id: 'p1', name: 'Real parent', status: 'archived', owner: null,
      description: '', areas: ['properties'], starts_at: null, ends_at: null,
      created_at: new Date('2026-09-09T00:00:00Z'), updated_at: null,
      project_type: 'listing', playbook_id: 'listing-onboarding', playbook_version: 1,
      person_id: null, property_id: null, contract_id: null,
    }]
  }
  const service = new ProjectService(new SqlProjectRepository(execute), capturingInfrastructure().infrastructure)
  const result = await service.execute({ operation: 'project.list', payload: {}, context: context() })
  assert.equal(result.ok, true)
  if (result.ok) {
    assert.equal(result.value[0].status, 'archived')
    assert.equal(result.value[0].createdAt, '2026-09-09T00:00:00.000Z')
  }
  assert.match(statement, /from project\s/)
  assert.doesNotMatch(statement, /wbs_project/)
})

test('Project authorization denial never reaches canonical persistence', async () => {
  let reads = 0
  const execute: QueryExecutor = async () => { reads++; return [] }
  const infra = capturingInfrastructure()
  infra.setAuthorized(false)
  const service = new ProjectService(new SqlProjectRepository(execute), infra.infrastructure)
  const result = await service.execute({ operation: 'project.list', payload: {}, context: context() })
  assert.equal(result.ok, false)
  assert.equal(reads, 0)
})

test('Catch-Up wiring uses canonical Project ingress and retains the WBS queue and calendar', () => {
  const page = readFileSync(new URL('../app/portal/catch-up/page.tsx', import.meta.url), 'utf8')
  const board = readFileSync(new URL('../components/portal/wbs/catch-up-board.tsx', import.meta.url), 'utf8')
  assert.match(page, /project.execute\(\{ operation: "project.list"/)
  assert.doesNotMatch(page, /wbs.execute\(\{ operation: "project\./)
  assert.match(page, /wbs.execute\(\{ operation: "wbs.listDue"/)
  assert.match(page, /errors: appServiceErrorSink\(\)/)
  assert.match(page, /if \(!dueRes.ok \|\| !projectRes.ok\)/)
  assert.match(board, /projects: Project\[\]/)
  assert.match(board, /<FullCalendarCandidate/)
})
