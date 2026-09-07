import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import { isWbsCategory } from './categories'
import type { WbsRepository } from './repository'
import { WBS_OPERATIONS, type WbsOperationMap } from './types'

/** Canonical WBS service: Work Items + Projects, built on the shared BaseService. */
export class WbsService extends BaseService<WbsOperationMap> {
  readonly domain = 'wbs'
  readonly version = '1'
  readonly description =
    'Owns work items (WBS nodes) and lightweight projects; follow-up is the key output.'
  protected readonly operations: ServiceOperationDefinitions<WbsOperationMap>

  constructor(
    private readonly repository: WbsRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [WBS_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one WBS work item by id.',
        authorization: 'wbs.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.id),
      },
      [WBS_OPERATIONS.LIST_DUE]: {
        kind: 'query',
        description: 'List open/due work items (optionally for one category) — the follow-up queue.',
        authorization: 'wbs.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.listDue(request),
      },
      [WBS_OPERATIONS.CREATE]: {
        kind: 'command',
        description: 'Create a work item (standalone follow-up or under a project).',
        authorization: 'wbs.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          this.assertValid(request)
          const item = await this.repository.create(request)
          await this.emit(
            {
              type: 'wbs.created',
              aggregateId: item.id,
              payload: { id: item.id, category: item.category, title: item.title },
            },
            context,
          )
          return item
        },
      },
      [WBS_OPERATIONS.SAVE]: {
        kind: 'command',
        description: 'Save an existing work item (title/notes/due/owner/category/order).',
        authorization: 'wbs.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          this.assertValid(request)
          const item = await this.repository.save(request)
          await this.emit(
            {
              type: 'wbs.updated',
              aggregateId: item.id,
              payload: { id: item.id, status: item.status, category: item.category },
            },
            context,
          )
          return item
        },
      },
      [WBS_OPERATIONS.COMPLETE]: {
        kind: 'command',
        description: 'Mark a work item done.',
        authorization: 'wbs.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          const item = await this.repository.complete(request)
          await this.emit(
            {
              type: 'wbs.completed',
              aggregateId: item.id,
              payload: { id: item.id, category: item.category },
            },
            context,
          )
          return item
        },
      },
      [WBS_OPERATIONS.CREATE_PROJECT]: {
        kind: 'command',
        description: 'Create a lightweight project (a WBS root).',
        authorization: 'wbs.write',
        execution: { mode: 'ordered', partitionBy: 'id' },
        handle: async (request, context) => {
          if (!request.name?.trim()) {
            this.fail('WBS_PROJECT_NAME_REQUIRED', 'A Project requires a name.')
          }
          const project = await this.repository.createProject(request)
          await this.emit(
            { type: 'project.created', aggregateId: project.id, payload: { id: project.id, name: project.name } },
            context,
          )
          return project
        },
      },
    }
  }

  private assertValid(
    request: { title: string; category: string; projectId?: string | null; parentId?: string | null },
  ): void {
    if (!request.title?.trim()) this.fail('WBS_TITLE_REQUIRED', 'A work item requires a title.')
    if (!isWbsCategory(request.category)) {
      this.fail('WBS_CATEGORY_UNKNOWN', `Unknown WBS category: ${request.category}`)
    }
  }

  invariants() {
    return [
      'WBS owns follow-up work items and lightweight projects; a WorkItem is open/doing/done/dismissed.',
      'Category is a fixed dimension (Clients, Contracts, Properties, Media, Marketing, Accounting, Management).',
      'A work item is either a project child (WBS) or a standalone ad-hoc follow-up (no project).',
      'WBS persistence is reachable only through the WbsRepository boundary.',
    ] as const
  }
}
