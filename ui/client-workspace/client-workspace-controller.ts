import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '../runtime'
import { projectClientWorkspaceChannels } from './channel-projection'
import {
  INITIAL_CLIENT_WORKSPACE_MODEL,
  type ClientWorkspaceIntentMap,
  type ClientWorkspacePageModel,
} from './model'
import type { ClientWorkspaceSource } from './source'

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export class ClientWorkspaceController extends BasePageController<
  ClientWorkspacePageModel,
  ClientWorkspaceIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    ClientWorkspacePageModel,
    ClientWorkspaceIntentMap
  >

  private searchTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly source: ClientWorkspaceSource,
    initialModel: ClientWorkspacePageModel = INITIAL_CLIENT_WORKSPACE_MODEL,
  ) {
    super({
      ...initialModel,
      list: [...initialModel.list],
      channels: [...initialModel.channels],
      propertyContext: initialModel.propertyContext
        ? {
            ...initialModel.propertyContext,
            properties: [...initialModel.propertyContext.properties],
          }
        : null,
    })

    this.operations = {
      'clientWorkspace.load': {
        description: 'Load the bounded client rail and select the first client when needed.',
        execution: 'latest',
        handle: async (_request, context) => this.loadList(context),
      },
      'clientWorkspace.queryChanged': {
        description: 'Update the client rail search and debounce a latest-wins reload.',
        execution: 'parallel',
        handle: async ({ query }, context) => {
          context.update((model) => ({ ...model, query, page: 1 }))
          this.scheduleSearchLoad()
        },
      },
      'clientWorkspace.previousPage': {
        description: 'Move the client rail to the previous page.',
        execution: 'serial',
        handle: async (_request, context) => {
          const page = Math.max(1, context.snapshot().page - 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'clientWorkspace.load', payload: {} })
        },
      },
      'clientWorkspace.nextPage': {
        description: 'Move the client rail to the next page.',
        execution: 'serial',
        handle: async (_request, context) => {
          const current = context.snapshot()
          const page = Math.min(current.pageCount, current.page + 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'clientWorkspace.load', payload: {} })
        },
      },
      'clientWorkspace.selectClient': {
        description: 'Set selection, then fan out independent Person, Property, and relationship lanes.',
        execution: 'parallel',
        handle: async ({ personId }, context) => {
          context.update((model) => ({
            ...model,
            selectedClientId: personId,
            client: null,
            channels: [],
            propertyContext: null,
            notesDraft: '',
            notesSaved: '',
            clientLoading: true,
            channelsLoading: true,
            propertyLoading: true,
            clientError: null,
            channelsError: null,
            propertyError: null,
            notesStatus: null,
          }))
          void this.dispatch({ operation: 'clientWorkspace.loadClient', payload: { personId } })
          void this.dispatch({ operation: 'clientWorkspace.loadChannels', payload: { personId } })
          void this.dispatch({ operation: 'clientWorkspace.loadPropertyContext', payload: { personId } })
        },
      },
      'clientWorkspace.loadClient': {
        description: 'Load the selected canonical Person independently of Property and relationship evidence.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadClient(personId, context),
      },
      'clientWorkspace.loadChannels': {
        description: 'Load and project the six relationship channels independently of Person and Property.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadChannels(personId, context),
      },
      'clientWorkspace.loadPropertyContext': {
        description: 'Load canonical Property relationships and Apple address evidence independently of Person detail.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadPropertyContext(personId, context),
      },
      'clientWorkspace.notesChanged': {
        description: 'Update the local selected-client notes draft only.',
        execution: 'parallel',
        handle: async ({ notes }, context) => {
          context.update((model) => ({ ...model, notesDraft: notes, notesStatus: null }))
        },
      },
      'clientWorkspace.saveNotes': {
        description: 'Persist the selected client notes and update the page model.',
        execution: 'serial',
        handle: async (_request, context) => this.saveNotes(context),
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
      void this.dispatch({ operation: 'clientWorkspace.load', payload: {} })
    }, 250)
  }

  private async loadList(context: PageOperationContext<ClientWorkspacePageModel>): Promise<void> {
    const { query, page, pageSize, selectedClientId } = context.snapshot()
    context.update((model) => ({ ...model, listLoading: true, listError: null }))

    try {
      const result = await this.source.loadList(
        { query, page, pageSize },
        { signal: context.signal },
      )
      const pageCount = Math.max(1, Math.ceil(result.total / result.pageSize))
      const selectedStillVisible = selectedClientId
        ? result.rows.some((row) => row.id === selectedClientId)
        : false
      const nextSelectedId = selectedStillVisible
        ? selectedClientId
        : result.rows[0]?.id ?? null

      context.update((model) => ({
        ...model,
        list: result.rows,
        total: result.total,
        page: result.page,
        pageSize: result.pageSize,
        pageCount,
        selectedClientId: nextSelectedId,
        listLoading: false,
        listError: null,
      }))

      if (
        context.isCurrent() &&
        nextSelectedId &&
        nextSelectedId !== selectedClientId
      ) {
        void this.dispatch({
          operation: 'clientWorkspace.selectClient',
          payload: { personId: nextSelectedId },
        })
      }
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        list: [],
        total: 0,
        pageCount: 1,
        listLoading: false,
        listError: message,
      }))
    }
  }

  private async loadClient(
    personId: string,
    context: PageOperationContext<ClientWorkspacePageModel>,
  ): Promise<void> {
    try {
      const client = await this.source.loadClient(personId, { signal: context.signal })
      if (!context.isCurrent() || context.snapshot().selectedClientId !== personId) return
      context.update((model) => ({
        ...model,
        client,
        notesDraft: client?.notes ?? '',
        notesSaved: client?.notes ?? '',
        clientLoading: false,
        clientError: client ? null : 'Client not found.',
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        client: null,
        clientLoading: false,
        clientError: message,
      }))
    }
  }

  private async loadChannels(
    personId: string,
    context: PageOperationContext<ClientWorkspacePageModel>,
  ): Promise<void> {
    try {
      const channels = await this.source.loadChannels(personId, { signal: context.signal })
      if (!context.isCurrent() || context.snapshot().selectedClientId !== personId) return
      context.update((model) => ({
        ...model,
        channels: projectClientWorkspaceChannels(channels),
        channelsLoading: false,
        channelsError: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        channels: projectClientWorkspaceChannels([]),
        channelsLoading: false,
        channelsError: message,
      }))
    }
  }

  private async loadPropertyContext(
    personId: string,
    context: PageOperationContext<ClientWorkspacePageModel>,
  ): Promise<void> {
    try {
      const propertyContext = await this.source.loadPropertyContext(personId, {
        signal: context.signal,
      })
      if (!context.isCurrent() || context.snapshot().selectedClientId !== personId) return
      context.update((model) => ({
        ...model,
        propertyContext,
        propertyLoading: false,
        propertyError: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        propertyContext: null,
        propertyLoading: false,
        propertyError: message,
      }))
    }
  }

  private async saveNotes(context: PageOperationContext<ClientWorkspacePageModel>): Promise<void> {
    const current = context.snapshot()
    if (!current.selectedClientId || current.notesDraft === current.notesSaved) return

    const personId = current.selectedClientId
    const notes = current.notesDraft
    context.update((model) => ({ ...model, notesSaving: true, notesStatus: null }))

    try {
      await this.source.saveNotes(personId, notes)
      context.update((model) => ({
        ...model,
        notesSaved: notes,
        notesSaving: false,
        notesStatus: 'Saved',
        client: model.client ? { ...model.client, notes } : model.client,
      }))
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        notesSaving: false,
        notesStatus: message,
      }))
    }
  }
}
