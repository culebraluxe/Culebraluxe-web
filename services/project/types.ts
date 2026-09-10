import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { WbsCategoryId } from '../wbs'
import type { PlaybookNodeDefinition } from './playbooks'

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
  projectType: string | null
  playbookId: string | null
  playbookVersion: number | null
  personId: string | null
  propertyId: string | null
  contractId: string | null
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
  projectType?: string | null
  playbookId?: string | null
  playbookVersion?: number | null
  personId?: string | null
  propertyId?: string | null
  contractId?: string | null
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
  projectType?: string | null
  playbookId?: string | null
  playbookVersion?: number | null
  personId?: string | null
  propertyId?: string | null
  contractId?: string | null
}

export type GetProjectRequest = { id: string }
export type ListProjectsRequest = Record<string, never>
export type CompleteProjectRequest = { id: string }
export type InstantiateProjectRequest = CreateProjectRequest & { playbookId: string; playbookVersion: number; nodes?: readonly PlaybookNodeDefinition[] }

export const PROJECT_OPERATIONS = {
  GET: 'project.get',
  LIST: 'project.list',
  CREATE: 'project.create',
  UPDATE: 'project.update',
  COMPLETE: 'project.complete',
  INSTANTIATE: 'project.instantiate',
} as const

export type ProjectOperationMap = {
  'project.get': { request: GetProjectRequest; response: Project | null }
  'project.list': { request: ListProjectsRequest; response: Project[] }
  'project.create': { request: CreateProjectRequest; response: Project }
  'project.update': { request: UpdateProjectRequest; response: Project }
  'project.complete': { request: CompleteProjectRequest; response: Project }
  'project.instantiate': { request: InstantiateProjectRequest; response: { project: Project; itemIds: string[] } }
}

export type ProjectOperationName = ServiceOperationName<ProjectOperationMap>
export type ProjectEnvelope<K extends ProjectOperationName = ProjectOperationName> =
  ServiceEnvelopeFor<ProjectOperationMap, K>
