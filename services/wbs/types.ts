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
export type DismissWbsItemRequest = { id: string }
export type ListWbsDueRequest = { category?: WbsCategoryId }
export type ListProjectWbsItemsRequest = Record<string, never>

export const WBS_OPERATIONS = {
  GET: 'wbs.get',
  LIST_DUE: 'wbs.listDue',
  LIST_PROJECT_ITEMS: 'wbs.listProjectItems',
  CREATE: 'wbs.create',
  SAVE: 'wbs.save',
  COMPLETE: 'wbs.complete',
  DISMISS: 'wbs.dismiss',
} as const

export type WbsOperationMap = {
  'wbs.get': { request: GetWbsItemRequest; response: WbsItem | null }
  'wbs.listDue': { request: ListWbsDueRequest; response: WbsItem[] }
  'wbs.listProjectItems': { request: ListProjectWbsItemsRequest; response: WbsItem[] }
  'wbs.create': { request: CreateWbsItemRequest; response: WbsItem }
  'wbs.save': { request: SaveWbsItemRequest; response: WbsItem }
  'wbs.complete': { request: CompleteWbsItemRequest; response: WbsItem }
  'wbs.dismiss': { request: DismissWbsItemRequest; response: WbsItem }
}

export type WbsOperationName = ServiceOperationName<WbsOperationMap>
export type WbsEnvelope<K extends WbsOperationName = WbsOperationName> =
  ServiceEnvelopeFor<WbsOperationMap, K>
