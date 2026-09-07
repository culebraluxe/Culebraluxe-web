import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import { isWbsCategory } from '../wbs'
import type { ProjectRepository } from './repository'
import { PROJECT_OPERATIONS, type ProjectOperationMap } from './types'

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
    }
  }

  invariants() {
    return [
      'Project is the parent; WBS work items are its children and may be standalone (no project).',
      'A Project may span multiple areas of the WBS category dimension.',
      'Project persistence is reachable only through the ProjectRepository boundary.',
    ] as const
  }
}
