import type {
  CompleteWbsItemRequest,
  CreateWbsItemRequest,
  CreateWbsProjectRequest,
  DismissWbsItemRequest,
  ListWbsDueRequest,
  SaveWbsItemRequest,
  WbsItem,
  WbsProject,
} from './types'

/** Persistence boundary for the WBS domain. Adapters own normalization. */
export interface WbsRepository {
  get(id: string): Promise<WbsItem | null>
  listDue(request: ListWbsDueRequest): Promise<WbsItem[]>
  create(request: CreateWbsItemRequest): Promise<WbsItem>
  save(request: SaveWbsItemRequest): Promise<WbsItem>
  complete(request: CompleteWbsItemRequest): Promise<WbsItem>
  dismiss(request: DismissWbsItemRequest): Promise<WbsItem>
  createProject(request: CreateWbsProjectRequest): Promise<WbsProject>
  getProject(id: string): Promise<WbsProject | null>
  listProjects(): Promise<WbsProject[]>
}
