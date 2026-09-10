import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  CompleteProjectRequest,
  CreateProjectRequest,
  Project,
  ProjectRepository,
  UpdateProjectRequest,
} from '@/services/project'
import type { WbsCategoryId } from '@/services/wbs'

type ProjectRow = {
  id: string
  name: string
  owner: string | null
  status: string
  description: string
  areas: unknown
  starts_at: unknown
  ends_at: unknown
  created_at: unknown
  updated_at: unknown
  project_type: string | null
  playbook_id: string | null
  playbook_version: unknown
  person_id: string | null
  property_id: string | null
  contract_id: string | null
}

function iso(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return new Date(String(value)).toISOString()
}

function areas(value: unknown): readonly WbsCategoryId[] {
  if (Array.isArray(value)) return value as WbsCategoryId[]
  if (typeof value === 'string' && value.startsWith('{')) {
    return value.slice(1, -1).split(',').filter(Boolean) as WbsCategoryId[]
  }
  return []
}

function num(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const n = Number(value)
  return Number.isInteger(n) ? n : null
}

function toProject(row: ProjectRow): Project {
  return {
    id: row.id,
    name: row.name,
    owner: row.owner,
    status: row.status as Project['status'],
    description: row.description,
    areas: areas(row.areas),
    startsAt: iso(row.starts_at),
    endsAt: iso(row.ends_at),
    createdAt: iso(row.created_at),
    updatedAt: iso(row.updated_at),
    projectType: row.project_type,
    playbookId: row.playbook_id,
    playbookVersion: num(row.playbook_version),
    personId: row.person_id,
    propertyId: row.property_id,
    contractId: row.contract_id,
  }
}

/** SQL adapter behind ProjectService (parent of WBS children). */
export class SqlProjectRepository implements ProjectRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(id: string): Promise<Project | null> {
    const rows = (await this.execute`
      select id, name, owner, status, description, areas, starts_at, ends_at, created_at, updated_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id
      from project where id = ${id} limit 1
    `) as unknown as ProjectRow[]
    return rows[0] ? toProject(rows[0]) : null
  }

  async list(): Promise<Project[]> {
    const rows = (await this.execute`
      select id, name, owner, status, description, areas, starts_at, ends_at, created_at, updated_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id
      from project order by created_at desc, id
    `) as unknown as ProjectRow[]
    return rows.map(toProject)
  }

  async create(request: CreateProjectRequest): Promise<Project> {
    const rows = (await this.execute`
      insert into project (id, name, owner, description, areas, starts_at, ends_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id)
      values (${request.id}, ${request.name}, ${request.owner ?? null}, ${request.description ?? ''},
              ${request.areas ?? []}, ${request.startsAt ?? null}, ${request.endsAt ?? null}, ${request.projectType ?? null}, ${request.playbookId ?? null}, ${request.playbookVersion ?? null}, ${request.personId ?? null}, ${request.propertyId ?? null}, ${request.contractId ?? null})
      returning id, name, owner, status, description, areas, starts_at, ends_at, created_at, updated_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id
    `) as unknown as ProjectRow[]
    if (!rows[0]) throw new Error('Project creation returned no row.')
    return toProject(rows[0])
  }

  async update(request: UpdateProjectRequest): Promise<Project> {
    const rows = (await this.execute`
      update project
      set name = coalesce(${request.name ?? null}, name),
          owner = coalesce(${request.owner ?? null}, owner),
          status = coalesce(${request.status ?? null}, status),
          description = coalesce(${request.description ?? null}, description),
          areas = coalesce(${request.areas ?? null}, areas),
          starts_at = coalesce(${request.startsAt ?? null}, starts_at),
          ends_at = coalesce(${request.endsAt ?? null}, ends_at),
          project_type = coalesce(${request.projectType ?? null}, project_type),
          playbook_id = coalesce(${request.playbookId ?? null}, playbook_id),
          playbook_version = coalesce(${request.playbookVersion ?? null}, playbook_version),
          person_id = coalesce(${request.personId ?? null}, person_id),
          property_id = coalesce(${request.propertyId ?? null}, property_id),
          contract_id = coalesce(${request.contractId ?? null}, contract_id),
          updated_at = now()
      where id = ${request.id}
      returning id, name, owner, status, description, areas, starts_at, ends_at, created_at, updated_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id
    `) as unknown as ProjectRow[]
    if (!rows[0]) throw new Error(`Project not found: ${request.id}`)
    return toProject(rows[0])
  }

  async complete(request: CompleteProjectRequest): Promise<Project> {
    const rows = (await this.execute`
      update project set status = 'done', updated_at = now()
      where id = ${request.id}
      returning id, name, owner, status, description, areas, starts_at, ends_at, created_at, updated_at, project_type, playbook_id, playbook_version, person_id, property_id, contract_id
    `) as unknown as ProjectRow[]
    if (!rows[0]) throw new Error(`Project not found: ${request.id}`)
    return toProject(rows[0])
  }
}
