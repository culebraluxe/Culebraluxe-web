import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { WbsCategoryId } from './categories'

export type WbsStatus = 'open' | 'doing' | 'done' | 'dismissed'

export type WbsEntityLink = {
  /**
   * The thing this work item is about. Polymorphic by design: work hangs off a
   * person, a property, a contract, or a transaction. `process_instance` is the
   * transaction (process_instances) — design doc section 6.5: WBS items hang off
   * the same transaction the engine's task-nodes run in. `deal` stays for legacy
   * rows; nothing is renamed until the reconcile step (section 7.6).
   */
  type: 'person' | 'property' | 'contract' | 'deal' | 'process_instance'
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
/** The tie: every work item hanging off one thing (a transaction, contract, ...). */
export type ListWbsForEntityRequest = { type: WbsEntityLink['type']; id: string }

export const WBS_OPERATIONS = {
  GET: 'wbs.get',
  LIST_DUE: 'wbs.listDue',
  LIST_PROJECT_ITEMS: 'wbs.listProjectItems',
  LIST_FOR_ENTITY: 'wbs.listForEntity',
  CREATE: 'wbs.create',
  SAVE: 'wbs.save',
  COMPLETE: 'wbs.complete',
  DISMISS: 'wbs.dismiss',
} as const

export type WbsOperationMap = {
  'wbs.get': { request: GetWbsItemRequest; response: WbsItem | null }
  'wbs.listDue': { request: ListWbsDueRequest; response: WbsItem[] }
  'wbs.listProjectItems': { request: ListProjectWbsItemsRequest; response: WbsItem[] }
  'wbs.listForEntity': { request: ListWbsForEntityRequest; response: WbsItem[] }
  'wbs.create': { request: CreateWbsItemRequest; response: WbsItem }
  'wbs.save': { request: SaveWbsItemRequest; response: WbsItem }
  'wbs.complete': { request: CompleteWbsItemRequest; response: WbsItem }
  'wbs.dismiss': { request: DismissWbsItemRequest; response: WbsItem }
}

export type WbsOperationName = ServiceOperationName<WbsOperationMap>
export type WbsEnvelope<K extends WbsOperationName = WbsOperationName> =
  ServiceEnvelopeFor<WbsOperationMap, K>
