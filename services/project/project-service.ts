import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import { isWbsCategory, type WbsOperationMap } from '../wbs'
import type { ProjectRepository } from './repository'
import { PROJECT_OPERATIONS, type ProjectOperationMap } from './types'
import { getPlaybook } from './playbooks'

/** Project service — the parent of WBS work items. Separate from wbs (separate table). */
export class ProjectService extends BaseService<ProjectOperationMap> {
  readonly domain = 'project'
  readonly version = '1'
  readonly description = 'Owns Projects; WBS work items hang beneath a Project as children.'
  protected readonly operations: ServiceOperationDefinitions<ProjectOperationMap>

  constructor(
    private readonly repository: ProjectRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [PROJECT_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one Project by id.',
        authorization: 'project.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.id),
      },
      [PROJECT_OPERATIONS.LIST]: {
        kind: 'query',
        description: 'List Projects (newest first).',
        authorization: 'project.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async () => this.repository.list(),
      },
      [PROJECT_OPERATIONS.CREATE]: {
        kind: 'command',
        description: 'Create a Project (parent of WBS work items).',
        authorization: 'project.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          if (!request.name?.trim()) this.fail('PROJECT_NAME_REQUIRED', 'A Project requires a name.')
          for (const area of request.areas ?? []) {
            if (!isWbsCategory(area)) this.fail('PROJECT_AREA_UNKNOWN', `Unknown area: ${area}`)
          }
          if (request.playbookVersion != null && (!Number.isInteger(request.playbookVersion) || request.playbookVersion < 1)) {
            this.fail('PROJECT_PLAYBOOK_VERSION_INVALID', 'Playbook version must be a positive integer.')
          }
          const project = await this.repository.create(request)
          await this.emit(
            { type: 'project.created', aggregateId: project.id, payload: { id: project.id, name: project.name } },
            context,
          )
          return project
        },
      },
      [PROJECT_OPERATIONS.UPDATE]: {
        kind: 'command',
        description: 'Update a Project (name/owner/status/dates/areas).',
        authorization: 'project.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          if (request.name !== undefined && !request.name.trim()) this.fail('PROJECT_NAME_REQUIRED', 'A Project requires a name.')
          for (const area of request.areas ?? []) {
            if (!isWbsCategory(area)) this.fail('PROJECT_AREA_UNKNOWN', `Unknown area: ${area}`)
          }
          if (request.playbookVersion != null && (!Number.isInteger(request.playbookVersion) || request.playbookVersion < 1)) {
            this.fail('PROJECT_PLAYBOOK_VERSION_INVALID', 'Playbook version must be a positive integer.')
          }
          const project = await this.repository.update(request)
          await this.emit(
            { type: 'project.updated', aggregateId: project.id, payload: { id: project.id, status: project.status } },
            context,
          )
          return project
        },
      },
      [PROJECT_OPERATIONS.COMPLETE]: {
        kind: 'command',
        description: 'Mark a Project done.',
        authorization: 'project.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          const project = await this.repository.complete(request)
          await this.emit(
            { type: 'project.completed', aggregateId: project.id, payload: { id: project.id, name: project.name } },
            context,
          )
          return project
        },
      },
      [PROJECT_OPERATIONS.INSTANTIATE]: {
        kind: 'command', description: 'Create a Project and instantiate its versioned WBS playbook.',
        authorization: 'project.write', execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          const playbook = getPlaybook(request.playbookId, request.playbookVersion)
          if (!playbook) this.fail('PROJECT_PLAYBOOK_NOT_FOUND', `Unknown playbook ${request.playbookId}@${request.playbookVersion}.`)
          const existing = await this.repository.get(request.id)
          if (existing) {
            if (existing.playbookId !== playbook.id || existing.playbookVersion !== playbook.version) {
              this.fail('PROJECT_PLAYBOOK_CONFLICT', `Project ${request.id} already exists with a different playbook.`)
            }
            const existingItems = await this.callService<WbsOperationMap, 'wbs.listProjectItems'>('wbs', 'wbs.listProjectItems', {}, context)
            return { project: existing, itemIds: existingItems.filter(item => item.projectId === existing.id).map(item => item.id) }
          }
          const nodes = request.nodes ?? playbook.nodes
          const keys = new Set<string>()
          for (const node of nodes) {
            if (!node.key.trim() || keys.has(node.key)) this.fail('PROJECT_PLAYBOOK_NODE_INVALID', `Playbook node key must be unique: ${node.key}`)
            if (!isWbsCategory(node.category)) this.fail('PROJECT_PLAYBOOK_NODE_INVALID', `Unknown playbook node category: ${node.category}`)
            if (node.parentKey && !nodes.some(candidate => candidate.key === node.parentKey)) {
              this.fail('PROJECT_PLAYBOOK_NODE_INVALID', `Parent node is missing: ${node.parentKey}`)
            }
            keys.add(node.key)
          }
          const visiting = new Set<string>()
          const visited = new Set<string>()
          const visit = (key: string): void => {
            if (visiting.has(key)) this.fail('PROJECT_PLAYBOOK_NODE_INVALID', `Playbook parent cycle includes: ${key}`)
            if (visited.has(key)) return
            visiting.add(key)
            const parent = nodes.find(candidate => candidate.key === key)?.parentKey
            if (parent) visit(parent)
            visiting.delete(key)
            visited.add(key)
          }
          for (const node of nodes) visit(node.key)
          const project = await this.repository.create({ ...request, projectType: playbook.projectType, playbookId: playbook.id, playbookVersion: playbook.version })
          const itemIds: string[] = []
          for (const node of nodes) {
            const id = `${project.id}-${node.key}`
            const parentId = node.parentKey ? `${project.id}-${node.parentKey}` : null
            const item = await this.callService<WbsOperationMap, 'wbs.create'>('wbs', 'wbs.create', {
              id, title: node.title, category: node.category, projectId: project.id,
              parentId, order: node.order,
            }, context)
            itemIds.push(item.id)
          }
          await this.emit({ type: 'project.created', aggregateId: project.id, payload: { id: project.id, name: project.name, playbookId: playbook.id, playbookVersion: playbook.version, itemCount: itemIds.length } }, context)
          return { project, itemIds }
        },
      },
    }
  }

  invariants() {
    return [
      'Project is the parent; WBS work items are its children and may be standalone (no project).',
      'A Project may span multiple areas of the WBS category dimension.',
      'Project persistence is reachable only through the ProjectRepository boundary.',
    ] as const
  }

  dependencies() {
    // Instantiation is intentionally routed through the WBS contract. Keeping
    // this explicit makes service discovery and startup validation truthful.
    return ['wbs'] as const
  }
}
