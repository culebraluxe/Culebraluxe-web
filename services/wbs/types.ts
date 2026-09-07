import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { WbsCategoryId } from './categories'

export type WbsStatus = 'open' | 'doing' | 'done' | 'dismissed'

export type WbsEntityLink = {
  type: 'person' | 'property' | 'contract' | 'deal'
  id: string
}

export type WbsItem = {
  id: string
  title: string
  notes: string
  category: WbsCategoryId
  status: WbsStatus
  projectId: string | null
  parentId: string | null
  dueAt: string | null
  owner: string | null
  order: number | null
  entity: WbsEntityLink | null
  createdAt: string | null
  updatedAt: string | null
}

export type WbsProject = {
  id: string
  name: string
  owner: string | null
  status: WbsStatus
  createdAt: string | null
  updatedAt: string | null
}

export type GetWbsItemRequest = { id: string }
export type CreateWbsItemRequest = {
  id: string
  title: string
  notes?: string
  category: WbsCategoryId
  projectId?: string | null
  parentId?: string | null
  dueAt?: string | null
  owner?: string | null
  order?: number | null
  entity?: WbsEntityLink | null
}
export type SaveWbsItemRequest = CreateWbsItemRequest & { status?: WbsStatus }
export type CompleteWbsItemRequest = { id: string }
export type ListWbsDueRequest = { category?: WbsCategoryId }
export type CreateWbsProjectRequest = { id: string; name: string; owner?: string | null }

export const WBS_OPERATIONS = {
  GET: 'wbs.get',
  LIST_DUE: 'wbs.listDue',
  CREATE: 'wbs.create',
  SAVE: 'wbs.save',
  COMPLETE: 'wbs.complete',
  CREATE_PROJECT: 'project.create',
} as const

export type WbsOperationMap = {
  'wbs.get': { request: GetWbsItemRequest; response: WbsItem | null }
  'wbs.listDue': { request: ListWbsDueRequest; response: WbsItem[] }
  'wbs.create': { request: CreateWbsItemRequest; response: WbsItem }
  'wbs.save': { request: SaveWbsItemRequest; response: WbsItem }
  'wbs.complete': { request: CompleteWbsItemRequest; response: WbsItem }
  'project.create': { request: CreateWbsProjectRequest; response: WbsProject }
}

export type WbsOperationName = ServiceOperationName<WbsOperationMap>
export type WbsEnvelope<K extends WbsOperationName = WbsOperationName> =
  ServiceEnvelopeFor<WbsOperationMap, K>
