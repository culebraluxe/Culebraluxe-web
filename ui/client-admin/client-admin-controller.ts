import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '../runtime'
import {
  INITIAL_CLIENT_ADMIN_MODEL,
  type ClientAdminIntentMap,
  type ClientAdminPageModel,
} from './model'
import type { ClientAdminSource } from './source'

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

/**
 * MVI/MVU controller for the existing real Client Administration view.
 * React emits intents; this controller owns loading/search/paging effects and
 * publishes dead-data PageModel snapshots back to the view.
 */
export class ClientAdminController extends BasePageController<
  ClientAdminPageModel,
  ClientAdminIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    ClientAdminPageModel,
    ClientAdminIntentMap
  >

  private searchTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly source: ClientAdminSource,
    initialModel: ClientAdminPageModel = INITIAL_CLIENT_ADMIN_MODEL,
  ) {
    super({ ...initialModel, rows: [...initialModel.rows] })

    this.operations = {
      'clientAdmin.load': {
        description: 'Load the current Client Administration page into the PageModel.',
        execution: 'latest',
        handle: async (_request, context) => this.load(context),
      },
      'clientAdmin.searchChanged': {
        description: 'Update search intent and debounce a latest-wins page load.',
        execution: 'parallel',
        handle: async ({ search }, context) => {
          context.update((model) => ({ ...model, search, page: 1 }))
          this.scheduleSearchLoad()
        },
      },
      'clientAdmin.previousPage': {
        description: 'Move to the previous page and reload.',
        execution: 'serial',
        handle: async (_request, context) => {
          const page = Math.max(1, context.snapshot().page - 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'clientAdmin.load', payload: {} })
        },
      },
      'clientAdmin.nextPage': {
        description: 'Move to the next page and reload.',
        execution: 'serial',
        handle: async (_request, context) => {
          const current = context.snapshot()
          const page = Math.min(current.pageCount, current.page + 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'clientAdmin.load', payload: {} })
        },
      },
    }
  }

  override dispose(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = null
    super.dispose()
  }

  private scheduleSearchLoad(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null
      void this.dispatch({ operation: 'clientAdmin.load', payload: {} })
    }, 250)
  }

  private async load(context: PageOperationContext<ClientAdminPageModel>): Promise<void> {
    const { search, page, pageSize } = context.snapshot()
    context.update((model) => ({ ...model, loading: true, error: null }))

    try {
      const result = await this.source.load(
        { search, page, pageSize },
        { signal: context.signal },
      )
      const pageCount = Math.max(1, Math.ceil(result.total / result.pageSize))
      context.update((model) => ({
        ...model,
        rows: result.rows,
        total: result.total,
        page: result.page,
        pageSize: result.pageSize,
        pageCount,
        loading: false,
        error: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      console.error('Failed to load client administration:', error)
      context.update((model) => ({
        ...model,
        rows: [],
        total: 0,
        pageCount: 1,
        loading: false,
        error: message,
      }))
    }
  }
}
