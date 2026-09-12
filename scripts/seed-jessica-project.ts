// ONE-OFF: create the canonical Projects record for Jessica Iverson in PROD, through
// the Project service (project.instantiate) so the playbook spine is created properly
// rather than hand-inserting a parent row.
//
//   node --env-file=.env.local --import tsx scripts/seed-jessica-project.ts prod
//
// Idempotent: project.instantiate returns the existing project unchanged when it
// already carries the same playbook.
import { randomUUID } from 'node:crypto'

import type { ServiceContext } from '../services/core'

// The database gateway builds its Neon executor at MODULE LOAD (not lazily), so
// APP_ENV must be set before the db modules are imported. A static import would run
// first and bind the connection to .env.local's development target - and dbTargetInfo()
// would still report "prod" afterwards, because it re-reads process.env at call time.
// That is exactly how a first attempt seeded DEV while reporting the PROD branch.
// Hence the dynamic imports inside main().
const target = process.argv[2]
if (target !== 'dev' && target !== 'prod') {
  console.error('usage: seed-jessica-project.ts <dev|prod>')
  process.exit(2)
}
process.env.APP_ENV = target === 'prod' ? 'production' : 'development'

const JESSICA_IVERSON = 'b741d639-3173-47bc-adff-769865c6347d'
const PROJECT_ID = 'jessica-iverson-listing'

async function main() {
  const { ServiceRegistry, dbTargetInfo } = await import('../services/core').then(async (core) => ({
    ...core,
    ...(await import('../db/client')),
  })) as { ServiceRegistry: typeof import('../services/core').ServiceRegistry } & {
    dbTargetInfo: typeof import('../db/client').dbTargetInfo
  }

  const { AuthorizationService, StaticAuthorizationPolicyProvider } = await import('../services/entitlement')
  const { ProjectService } = await import('../services/project')
  const { WbsService } = await import('../services/wbs')
  const { SqlProjectRepository } = await import('../db/project-service-repository')
  const { SqlWbsRepository } = await import('../db/wbs-service-repository')

  const resolved = dbTargetInfo()
  console.log(`resolved target: ${resolved.target} (branch ${resolved.neonBranch})`)
  if (resolved.target !== target) {
    console.error(`refusing to run: resolved ${resolved.target} but ${target} was requested`)
    process.exitCode = 1
    return
  }

  const infrastructure = {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
  }
  const registry = new ServiceRegistry()
  registry.register(new WbsService(new SqlWbsRepository(), { ...infrastructure, router: registry }))
  const project = registry.register(
    new ProjectService(new SqlProjectRepository(), { ...infrastructure, router: registry }),
  )

  // A one-off seeded record needs no invented human operator; the level is the
  // ordinary business user so nothing here claims ROOT.
  const context: ServiceContext = {
    actor: { id: 'system:seed-jessica-project', kind: 'user' },
    correlationId: randomUUID(),
    principal: {
      appUserId: 'system:seed-jessica-project',
      level: 'BUSINESS_POWER_USER',
      roleCodes: ['BUSINESS_POWER_USER'],
    },
  }

  const existing = await project.execute({
    operation: 'project.get',
    payload: { id: PROJECT_ID },
    context,
  })
  if (!existing.ok) {
    console.error(`project.get failed: ${existing.error.code} ${existing.error.message}`)
    process.exitCode = 1
    return
  }
  if (existing.value) {
    console.log(`project ${PROJECT_ID} already exists: "${existing.value.name}" status=${existing.value.status}`)
    return
  }

  const result = await project.execute({
    operation: 'project.instantiate',
    payload: {
      id: PROJECT_ID,
      name: 'Jessica Iverson Listing',
      description: 'Listing project for Jessica Iverson.',
      personId: JESSICA_IVERSON,
      areas: ['clients', 'properties', 'contracts', 'media', 'marketing', 'accounting'],
      playbookId: 'listing-onboarding',
      playbookVersion: 1,
    },
    context,
  })

  if (!result.ok) {
    console.error(`project.instantiate failed: ${result.error.code} ${result.error.message}`)
    process.exitCode = 1
    return
  }
  console.log(`created project ${result.value.project.id}: "${result.value.project.name}"`)
  console.log(`  status=${result.value.project.status} personId=${result.value.project.personId}`)
  console.log(`  playbook=${result.value.project.playbookId}@${result.value.project.playbookVersion}`)
  console.log(`  wbs items created (${result.value.itemIds.length}): ${result.value.itemIds.join(', ')}`)
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
