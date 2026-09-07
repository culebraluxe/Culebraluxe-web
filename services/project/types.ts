import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { WbsCategoryId } from '../wbs'

/** Project is the parent; WBS nodes are its children. */
export type ProjectStatus = 'open' | 'doing' | 'done' | 'archived'

export type Project = {
  id: string
  name: string
  owner: string | null
  status: ProjectStatus
  description: string
  /** Areas a project spans (the WBS category dimension), optional for now. */
  areas: readonly WbsCategoryId[]
  startsAt: string | null
  endsAt: string | null
  createdAt: string | null
  updatedAt: string | null
}

export type CreateProjectRequest = {
  id: string
  name: string
  owner?: string | null
  description?: string
  areas?: readonly WbsCategoryId[]
  startsAt?: string | null
  endsAt?: string | null
}

export type UpdateProjectRequest = {
  id: string
  name?: string
  owner?: string | null
  status?: ProjectStatus
  description?: string
  areas?: readonly WbsCategoryId[]
  startsAt?: string | null
  endsAt?: string | null
}

export type GetProjectRequest = { id: string }
export type ListProjectsRequest = Record<string, never>
export type CompleteProjectRequest = { id: string }

export const PROJECT_OPERATIONS = {
  GET: 'project.get',
  LIST: 'project.list',
  CREATE: 'project.create',
  UPDATE: 'project.update',
  COMPLETE: 'project.complete',
} as const

export type ProjectOperationMap = {
  'project.get': { request: GetProjectRequest; response: Project | null }
  'project.list': { request: ListProjectsRequest; response: Project[] }
  'project.create': { request: CreateProjectRequest; response: Project }
  'project.update': { request: UpdateProjectRequest; response: Project }
  'project.complete': { request: CompleteProjectRequest; response: Project }
}

export type ProjectOperationName = ServiceOperationName<ProjectOperationMap>
export type ProjectEnvelope<K extends ProjectOperationName = ProjectOperationName> =
  ServiceEnvelopeFor<ProjectOperationMap, K>
