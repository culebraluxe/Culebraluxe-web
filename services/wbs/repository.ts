import type {
  CompleteWbsItemRequest,
  CreateWbsItemRequest,
  DismissWbsItemRequest,
  ListWbsDueRequest,
  ListProjectWbsItemsRequest,
  SaveWbsItemRequest,
  WbsItem,
} from './types'

/** Persistence boundary for the WBS domain. Adapters own normalization. */
export interface WbsRepository {
  get(id: string): Promise<WbsItem | null>
  listDue(request: ListWbsDueRequest): Promise<WbsItem[]>
  listProjectItems(request: ListProjectWbsItemsRequest): Promise<WbsItem[]>
  create(request: CreateWbsItemRequest): Promise<WbsItem>
  save(request: SaveWbsItemRequest): Promise<WbsItem>
  complete(request: CompleteWbsItemRequest): Promise<WbsItem>
  dismiss(request: DismissWbsItemRequest): Promise<WbsItem>
}
