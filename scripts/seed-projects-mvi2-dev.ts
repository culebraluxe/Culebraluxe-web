import { randomUUID } from 'node:crypto'

import { SqlProjectRepository } from '../db/project-service-repository'
import { SqlWbsRepository } from '../db/wbs-service-repository'
import { ProjectService } from '../services/project'
import { WbsService } from '../services/wbs'
import { AuthorizationService, StaticAuthorizationPolicyProvider } from '../services/entitlement'
import { resolveSecurityLevel } from '../services/security'
import { mapRealProjectsToWorkspace } from '../ui/projects/service-projection'
import { ServiceRegistry } from '../services/core'

const PROJECT_ID = 'mvi2-fake-listing-20260909'
const OWNER_ID = 'mvi2-test-owner'
const context = {
  actor: { id: OWNER_ID, kind: 'user' as const },
  correlationId: randomUUID(),
  principal: { appUserId: OWNER_ID, level: resolveSecurityLevel(['root']), roleCodes: ['root'] },
}

async function main() {
  if (process.env.APP_ENV === 'production' || process.env.VERCEL_ENV === 'production') {
    throw new Error('Refusing to seed MVI-02 test data into production.')
  }

  const infrastructure = {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
  }
  const wbsService = new WbsService(new SqlWbsRepository(), infrastructure)
  const registry = new ServiceRegistry()
  const projectService = registry.register(new ProjectService(new SqlProjectRepository(), { ...infrastructure, router: registry }))
  registry.register(wbsService)

  const lookupResult = await projectService.execute({
    operation: 'project.get', payload: { id: PROJECT_ID }, context,
  })
  if (!lookupResult.ok) throw new Error(`Project lookup failed: ${lookupResult.error.code}`)
  let project = lookupResult.value
  if (!project) {
    const instantiated = await projectService.execute({
      operation: 'project.instantiate', payload: {
        id: PROJECT_ID, name: 'MVI-02 Synthetic Listing Test', owner: OWNER_ID,
        description: 'DEV-only synthetic project proving ProjectService + WbsService wiring.',
        areas: ['clients', 'properties', 'contracts', 'media', 'marketing', 'accounting'],
        playbookId: 'listing-onboarding', playbookVersion: 1,
        personId: 'mvi2-fake-person-20260909',
        propertyId: 'mvi2-fake-property-20260909',
        contractId: 'mvi2-fake-contract-20260909',
      }, context,
    })
    if (!instantiated.ok) throw new Error(`Project instantiation failed: ${instantiated.error.code}`)
    project = instantiated.value.project
  }

  const items = [
    { id: `${PROJECT_ID}-parties`, title: 'Clients / Parties', category: 'clients' as const, order: 1 },
    { id: `${PROJECT_ID}-property`, title: 'Property', category: 'properties' as const, order: 2 },
    { id: `${PROJECT_ID}-agreement`, title: 'Listing Agreement', category: 'contracts' as const, order: 3 },
    { id: `${PROJECT_ID}-signature`, title: 'Seller Signature', category: 'contracts' as const, parentId: `${PROJECT_ID}-agreement`, order: 1 },
    { id: `${PROJECT_ID}-media`, title: 'Cabinet + Photos', category: 'media' as const, order: 4 },
    { id: `${PROJECT_ID}-marketing`, title: 'Coming Soon / Marketing', category: 'marketing' as const, order: 5 },
    { id: `${PROJECT_ID}-accounting`, title: 'Commission / Accounting', category: 'accounting' as const, order: 6 },
  ]
  const created: string[] = []
  for (const item of items) {
    const existing = await wbsService.execute({ operation: 'wbs.get', payload: { id: item.id }, context })
    if (!existing.ok) throw new Error(`WBS lookup failed for ${item.id}: ${existing.error.code}`)
    if (!existing.value) {
      const result = await wbsService.execute({ operation: 'wbs.create', payload: { ...item, projectId: PROJECT_ID }, context })
      if (!result.ok) throw new Error(`WBS creation failed for ${item.id}: ${result.error.code}`)
      created.push(item.id)
    }
  }

  for (const id of [`${PROJECT_ID}-parties`, `${PROJECT_ID}-property`]) {
    const item = await wbsService.execute({ operation: 'wbs.get', payload: { id }, context })
    if (!item.ok) throw new Error(`WBS status lookup failed for ${id}: ${item.error.code}`)
    if (item.value?.status === 'open') {
      const done = await wbsService.execute({ operation: 'wbs.complete', payload: { id }, context })
      if (!done.ok) throw new Error(`WBS completion failed for ${id}: ${done.error.code}`)
    }
  }

  const doing = await projectService.execute({ operation: 'project.update', payload: { id: PROJECT_ID, status: 'doing' }, context })
  if (!doing.ok) throw new Error(`Project status update failed: ${doing.error.code}`)
  const [readProject, readItems] = await Promise.all([
    projectService.execute({ operation: 'project.get', payload: { id: PROJECT_ID }, context }),
    wbsService.execute({ operation: 'wbs.listProjectItems', payload: {}, context }),
  ])
  if (!readProject.ok || !readProject.value || !readItems.ok) throw new Error('Service readback failed.')
  const workspace = mapRealProjectsToWorkspace([readProject.value], readItems.value)
  const plan = workspace.poles.flatMap(pole => pole.projects).find(p => p.id === PROJECT_ID)
  console.log(JSON.stringify({
    databaseTarget: 'dev', project: readProject.value,
    createdItemCount: created.length, itemCount: readItems.value.filter(item => item.projectId === PROJECT_ID).length,
    workPlan: { progress: plan?.progress, status: plan?.status, phaseLabel: plan?.phaseLabel, nodes: plan?.workNodes.map(node => ({ title: node.title, status: node.status, childCount: node.children?.length ?? 0 })) },
  }, null, 2))
}

main().catch(error => { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1 })
