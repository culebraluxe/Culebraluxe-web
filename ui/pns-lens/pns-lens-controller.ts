import type { PnsCanonicalFields, PnsCanonicalSnapshot } from '@/lib/forms/pns-canonical-types'
import type { TemplateDefinition } from '@/lib/forms/template-types'
import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '@/ui/runtime'
import {
  buildPnsLensModel,
  type PnsLensIntentMap,
  type PnsLensModel,
} from './model'
import type { PnsLensSource } from './source'

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export class PnsLensController extends BasePageController<PnsLensModel, PnsLensIntentMap> {
  protected readonly operations: PageOperationDefinitions<PnsLensModel, PnsLensIntentMap>
  private searchTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly source: PnsLensSource,
    private readonly template: TemplateDefinition,
  ) {
    super(buildPnsLensModel(template))

    this.operations = {
      'pnsLens.load': {
        description: 'Load the bounded Person rail for P&S canonical work.',
        execution: 'latest',
        handle: async (_request, context) => this.loadList(context),
      },
      'pnsLens.queryChanged': {
        description: 'Search canonical Persons and debounce the latest rail request.',
        execution: 'parallel',
        handle: async ({ query }, context) => {
          context.update((model) => ({ ...model, query, page: 1 }))
          this.scheduleSearchLoad()
        },
      },
      'pnsLens.previousPage': {
        description: 'Move to the previous Person page.',
        execution: 'serial',
        handle: async (_request, context) => {
          context.update((model) => ({ ...model, page: Math.max(1, model.page - 1) }))
          void this.dispatch({ operation: 'pnsLens.load', payload: {} })
        },
      },
      'pnsLens.nextPage': {
        description: 'Move to the next Person page.',
        execution: 'serial',
        handle: async (_request, context) => {
          context.update((model) => ({ ...model, page: Math.min(model.pageCount, model.page + 1) }))
          void this.dispatch({ operation: 'pnsLens.load', payload: {} })
        },
      },
      'pnsLens.selectPerson': {
        description: 'Select the P&S anchor Person then fan out canonical context and Contract hydration.',
        execution: 'parallel',
        handle: async ({ personId }, context) => {
          context.update((model) => ({
            ...model,
            selectedPersonId: personId,
            selectedPropertyId: null,
            client: null,
            propertyContext: null,
            canonical: null,
            manualFields: [],
            fields: model.fields.map((field) => ({ ...field, value: '', origin: 'empty' })),
            contextLoading: true,
            bindingLoading: true,
            saving: false,
            contextError: null,
            bindingError: null,
            saveStatus: null,
          }))
          void this.dispatch({ operation: 'pnsLens.loadContext', payload: { personId } })
          void this.dispatch({ operation: 'pnsLens.loadBinding', payload: { personId } })
        },
      },
      'pnsLens.loadContext': {
        description: 'Load Person and Person→Property canonical context independently of Contract.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadContext(personId, context),
      },
      'pnsLens.loadBinding': {
        description: 'Hydrate P&S from Contract first, canonical Property/Person second, old Form evidence last.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadBinding(personId, context),
      },
      'pnsLens.selectProperty': {
        description: 'Choose the canonical subject Property for the P&S Contract draft.',
        execution: 'parallel',
        handle: async ({ propertyId }, context) => {
          context.update((model) => ({ ...model, selectedPropertyId: propertyId, saveStatus: null }))
        },
      },
      'pnsLens.fieldChanged': {
        description: 'Edit one local P&S field; canonical mutation remains an explicit save.',
        execution: 'parallel',
        handle: async ({ field, value }, context) => {
          context.update((model) => ({
            ...model,
            fields: model.fields.map((candidate) =>
              candidate.definition.name === field
                ? { ...candidate, value, origin: 'manual' }
                : candidate,
            ),
            manualFields: model.manualFields.includes(field)
              ? model.manualFields
              : [...model.manualFields, field],
            saveStatus: null,
          }))
        },
      },
      'pnsLens.ownerChanged': {
        description: 'Filter the P&S architecture lens by canonical owner.',
        handle: ({ owner }, context) => {
          context.update((model) => ({ ...model, ownerFilter: owner }))
        },
      },
      'pnsLens.reset': {
        description: 'Discard local edits and restore the last canonical Contract projection.',
        execution: 'serial',
        handle: (_request, context) => {
          context.update((model) => this.applySnapshot(model, model.canonical, true))
        },
      },
      'pnsLens.save': {
        description: 'Round-trip reviewed P&S fields through Property/Person/Firm and canonical Contract draft truth.',
        execution: 'serial',
        handle: async (_request, context) => this.save(context),
      },
    }
  }

  override dispose(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = null
    super.dispose()
  }

  private applySnapshot(
    model: PnsLensModel,
    snapshot: PnsCanonicalSnapshot | null,
    clearManual = false,
  ): PnsLensModel {
    if (!snapshot) return model
    const manual = clearManual ? new Set<string>() : new Set(model.manualFields)
    return {
      ...model,
      canonical: snapshot,
      selectedPropertyId: snapshot.physicalPropertyId ?? model.selectedPropertyId,
      fields: model.fields.map((field) => {
        const name = field.definition.name
        if (manual.has(name)) return field
        const value = snapshot.fields[name] ?? ''
        return { ...field, value, origin: snapshot.origins[name] ?? 'empty' }
      }),
      manualFields: clearManual ? [] : model.manualFields,
    }
  }

  private scheduleSearchLoad(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null
      void this.dispatch({ operation: 'pnsLens.load', payload: {} })
    }, 250)
  }

  private async loadList(context: PageOperationContext<PnsLensModel>): Promise<void> {
    const { query, page, pageSize, selectedPersonId } = context.snapshot()
    context.update((model) => ({ ...model, listLoading: true, listError: null }))
    try {
      const result = await this.source.loadList({ query, page, pageSize }, { signal: context.signal })
      const pageCount = Math.max(1, Math.ceil(result.total / result.pageSize))
      const selectedStillVisible = selectedPersonId
        ? result.rows.some((row) => row.id === selectedPersonId)
        : false
      const nextSelectedId = selectedStillVisible ? selectedPersonId : result.rows[0]?.id ?? null
      context.update((model) => ({
        ...model,
        list: result.rows,
        total: result.total,
        page: result.page,
        pageSize: result.pageSize,
        pageCount,
        selectedPersonId: nextSelectedId,
        listLoading: false,
        listError: null,
      }))
      if (nextSelectedId && nextSelectedId !== selectedPersonId) {
        void this.dispatch({ operation: 'pnsLens.selectPerson', payload: { personId: nextSelectedId } })
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

  private async loadContext(
    personId: string,
    context: PageOperationContext<PnsLensModel>,
  ): Promise<void> {
    try {
      const [client, propertyContext] = await Promise.all([
        this.source.loadPerson(personId, { signal: context.signal }),
        this.source.loadPropertyContext(personId, { signal: context.signal }),
      ])
      if (!context.isCurrent() || context.snapshot().selectedPersonId !== personId) return
      context.update((model) => ({
        ...model,
        client,
        propertyContext,
        contextLoading: false,
        contextError: client ? null : 'Person not found.',
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        client: null,
        propertyContext: null,
        contextLoading: false,
        contextError: message,
      }))
    }
  }

  private async loadBinding(
    personId: string,
    context: PageOperationContext<PnsLensModel>,
  ): Promise<void> {
    try {
      const canonical = await this.source.loadBinding(personId, null, { signal: context.signal })
      if (!context.isCurrent() || context.snapshot().selectedPersonId !== personId) return
      context.update((model) => ({
        ...this.applySnapshot(model, canonical, true),
        bindingLoading: false,
        bindingError: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({
        ...model,
        canonical: null,
        bindingLoading: false,
        bindingError: message,
      }))
    }
  }

  private async save(context: PageOperationContext<PnsLensModel>): Promise<void> {
    const model = context.snapshot()
    if (!model.selectedPersonId) return
    const fields = Object.fromEntries(
      model.fields.map((field) => [field.definition.name, field.value]),
    ) as PnsCanonicalFields

    context.update((current) => ({
      ...current,
      saving: true,
      bindingError: null,
      saveStatus: 'Saving canonical P&S draft…',
    }))
    try {
      const canonical = await this.source.saveBinding({
        personId: model.selectedPersonId,
        contractId: model.canonical?.contractId ?? null,
        physicalPropertyId: model.selectedPropertyId ?? model.canonical?.physicalPropertyId ?? null,
        fields,
      })
      context.update((current) => ({
        ...this.applySnapshot(current, canonical, true),
        saving: false,
        bindingError: null,
        saveStatus: `Contract ${canonical.contractId ?? 'draft'} saved as canonical working truth.`,
      }))
      void this.dispatch({ operation: 'pnsLens.loadContext', payload: { personId: model.selectedPersonId } })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      context.update((current) => ({
        ...current,
        saving: false,
        bindingError: message,
        saveStatus: null,
      }))
    }
  }
}
