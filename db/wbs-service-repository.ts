import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { WbsCategoryId, WbsEntityLink } from '@/services/wbs'
import type {
  CompleteWbsItemRequest,
  CreateWbsItemRequest,
  CreateWbsProjectRequest,
  DismissWbsItemRequest,
  ListWbsDueRequest,
  SaveWbsItemRequest,
  WbsItem,
  WbsProject,
  WbsRepository,
} from '@/services/wbs'

type WbsItemRow = {
  id: string
  project_id: string | null
  parent_id: string | null
  title: string
  notes: string
  category: string
  status: string
  due_at: unknown
  owner: string | null
  sort_order: unknown
  entity_type: string | null
  entity_id: string | null
  created_at: unknown
  updated_at: unknown
}

type WbsProjectRow = {
  id: string
  name: string
  owner: string | null
  status: string
  created_at: unknown
  updated_at: unknown
}

const ITEM_COLS =
  'id, project_id, parent_id, title, notes, category, status, ' +
  'due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at'

function iso(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return new Date(String(value)).toISOString()
}

function num(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const n = Number(value)
  return Number.isNaN(n) ? null : n
}

function toItem(row: WbsItemRow): WbsItem {
  return {
    id: row.id,
    projectId: row.project_id,
    parentId: row.parent_id,
    title: row.title,
    notes: row.notes,
    category: row.category as WbsCategoryId,
    status: row.status as WbsItem['status'],
    dueAt: iso(row.due_at),
    owner: row.owner,
    order: num(row.sort_order),
    entity:
      row.entity_type && row.entity_id
        ? ({ type: row.entity_type, id: row.entity_id } as WbsEntityLink)
        : null,
    createdAt: iso(row.created_at),
    updatedAt: iso(row.updated_at),
  }
}

function toProject(row: WbsProjectRow): WbsProject {
  return {
    id: row.id,
    name: row.name,
    owner: row.owner,
    status: row.status as WbsItem['status'],
    createdAt: iso(row.created_at),
    updatedAt: iso(row.updated_at),
  }
}


/** SQL adapter behind WbsService. */
export class SqlWbsRepository implements WbsRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(id: string): Promise<WbsItem | null> {
    const rows = (await this.execute`
      select id, project_id, parent_id, title, notes, category, status,
             due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
      from wbs_item where id = ${id} limit 1
    `) as unknown as WbsItemRow[]
    return rows[0] ? toItem(rows[0]) : null
  }

  async listDue(request: ListWbsDueRequest): Promise<WbsItem[]> {
    const rows = request.category
      ? ((await this.execute`
          select id, project_id, parent_id, title, notes, category, status,
                 due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
          from wbs_item where status in ('open', 'doing') and category = ${request.category}
          order by due_at nulls last, id
        `) as unknown as WbsItemRow[])
      : ((await this.execute`
          select id, project_id, parent_id, title, notes, category, status,
                 due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
          from wbs_item where status in ('open', 'doing')
          order by due_at nulls last, id
        `) as unknown as WbsItemRow[])
    return rows.map(toItem)
  }

  async create(request: CreateWbsItemRequest): Promise<WbsItem> {
    const rows = (await this.execute`
      insert into wbs_item (id, project_id, parent_id, title, notes, category, status,
                            due_at, owner, sort_order, entity_type, entity_id)
      values (${request.id}, ${request.projectId ?? null}, ${request.parentId ?? null},
              ${request.title}, ${request.notes ?? ''}, ${request.category}, 'open',
              ${request.dueAt ?? null}, ${request.owner ?? null}, ${request.order ?? null},
              ${request.entity?.type ?? null}, ${request.entity?.id ?? null})
      returning id, project_id, parent_id, title, notes, category, status,
                due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
    `) as unknown as WbsItemRow[]
    if (!rows[0]) throw new Error('WBS item creation returned no row.')
    return toItem(rows[0])
  }


  async save(request: SaveWbsItemRequest): Promise<WbsItem> {
    const rows = (await this.execute`
      update wbs_item
      set title = ${request.title},
          notes = ${request.notes ?? ''},
          category = ${request.category},
          status = coalesce(${request.status ?? null}, status),
          due_at = ${request.dueAt ?? null},
          owner = ${request.owner ?? null},
          sort_order = ${request.order ?? null},
          entity_type = ${request.entity?.type ?? null},
          entity_id = ${request.entity?.id ?? null},
          project_id = ${request.projectId ?? null},
          parent_id = ${request.parentId ?? null},
          updated_at = now()
      where id = ${request.id}
      returning id, project_id, parent_id, title, notes, category, status,
                due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
    `) as unknown as WbsItemRow[]
    if (!rows[0]) throw new Error(`WBS item not found: ${request.id}`)
    return toItem(rows[0])
  }

  async complete(request: CompleteWbsItemRequest): Promise<WbsItem> {
    const rows = (await this.execute`
      update wbs_item set status = 'done', updated_at = now()
      where id = ${request.id}
      returning id, project_id, parent_id, title, notes, category, status,
                due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
    `) as unknown as WbsItemRow[]
    if (!rows[0]) throw new Error(`WBS item not found: ${request.id}`)
    return toItem(rows[0])
  }

  async dismiss(request: DismissWbsItemRequest): Promise<WbsItem> {
    const rows = (await this.execute`
      update wbs_item set status = 'dismissed', updated_at = now()
      where id = ${request.id}
      returning id, project_id, parent_id, title, notes, category, status,
                due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
    `) as unknown as WbsItemRow[]
    if (!rows[0]) throw new Error(`WBS item not found: ${request.id}`)
    return toItem(rows[0])
  }


  async createProject(request: CreateWbsProjectRequest): Promise<WbsProject> {
    const rows = (await this.execute`
      insert into wbs_project (id, name, owner)
      values (${request.id}, ${request.name}, ${request.owner ?? null})
      returning id, name, owner, status, created_at, updated_at
    `) as unknown as WbsProjectRow[]
    if (!rows[0]) throw new Error('WBS project creation returned no row.')
    return toProject(rows[0])
  }

  async getProject(id: string): Promise<WbsProject | null> {
    const rows = (await this.execute`
      select id, name, owner, status, created_at, updated_at
      from wbs_project where id = ${id} limit 1
    `) as unknown as WbsProjectRow[]
    return rows[0] ? toProject(rows[0]) : null
  }

  async listProjects(): Promise<WbsProject[]> {
    const rows = (await this.execute`
      select id, name, owner, status, created_at, updated_at
      from wbs_project order by created_at desc, id
    `) as unknown as WbsProjectRow[]
    return rows.map(toProject)
  }
}

