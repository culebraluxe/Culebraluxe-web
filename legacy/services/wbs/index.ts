export { WbsService } from '@/legacy/services/wbs/wbs-service'
export type { WbsRepository } from '@/legacy/services/wbs/repository'
export {
  WBS_CATEGORIES,
  isWbsCategory,
  wbsCategoryLabel,
  MANAGEMENT_CATEGORY_ID,
  type WbsCategory,
  type WbsCategoryId,
} from '@/legacy/services/wbs/categories'
export {
  WBS_OPERATIONS,
  type CreateWbsItemRequest,
  type DismissWbsItemRequest,
  type GetWbsItemRequest,
  type ListProjectWbsItemsRequest,
  type ListWbsForEntityRequest,
  type ListWbsDueRequest,
  type SaveWbsItemRequest,
  type CompleteWbsItemRequest,
  type WbsEnvelope,
  type WbsEntityLink,
  type WbsItem,
  type WbsOperationMap,
  type WbsOperationName,
  type WbsStatus,
} from '@/legacy/services/wbs/types'
