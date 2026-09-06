import type { TemplateDefinition } from '@/lib/forms/template-types'
import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '../runtime'
import { projectListingAgreement } from './listing-projection'
import {
  INITIAL_FORM_LENS_MODEL,
  type FormLensIntentMap,
  type FormLensPageModel,
} from './model'
import type { FormLensSource } from './source'

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export class FormLensController extends BasePageController<
  FormLensPageModel,
  FormLensIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    FormLensPageModel,
    FormLensIntentMap
  >

  private searchTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly source: FormLensSource,
    private readonly template: TemplateDefinition,
    initialModel: FormLensPageModel = INITIAL_FORM_LENS_MODEL,
  ) {
    super({
      ...initialModel,
      list: [...initialModel.list],
      fields: [...initialModel.fields],
      manualFields: [...initialModel.manualFields],
      propertyContext: initialModel.propertyContext
        ? {
            ...initialModel.propertyContext,
            properties: [...initialModel.propertyContext.properties],
            observedAddresses: [...initialModel.propertyContext.observedAddresses],
          }
        : null,
    })

    this.operations = {
      'formLens.load': {
        description: 'Load the bounded Person rail and select the first Person when needed.',
        execution: 'latest',
        handle: async (_request, context) => this.loadList(context),
      },
      'formLens.queryChanged': {
        description: 'Update Person search and debounce a latest-wins rail reload.',
        execution: 'parallel',
        handle: async ({ query }, context) => {
          context.update((model) => ({ ...model, query, page: 1 }))
          this.scheduleSearchLoad()
        },
      },
      'formLens.previousPage': {
        description: 'Move the Person rail to the previous page.',
        execution: 'serial',
        handle: async (_request, context) => {
          const page = Math.max(1, context.snapshot().page - 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'formLens.load', payload: {} })
        },
      },
      'formLens.nextPage': {
        description: 'Move the Person rail to the next page.',
        execution: 'serial',
        handle: async (_request, context) => {
          const current = context.snapshot()
          const page = Math.min(current.pageCount, current.page + 1)
          context.update((model) => ({ ...model, page }))
          void this.dispatch({ operation: 'formLens.load', payload: {} })
        },
      },
      'formLens.selectPerson': {
        description: 'Set seller selection and fan out independent Person and Property lanes.',
        execution: 'parallel',
        handle: async ({ personId }, context) => {
          context.update((model) => this.withProjection({
            ...model,
            selectedPersonId: personId,
            selectedPropertyId: null,
            client: null,
            propertyContext: null,
            manualFields: [],
            clientLoading: true,
            propertyLoading: true,
            clientError: null,
            propertyError: null,
          }, null, true))
          void this.dispatch({ operation: 'formLens.loadPerson', payload: { personId } })
          void this.dispatch({ operation: 'formLens.loadPropertyContext', payload: { personId } })
        },
      },
      'formLens.loadPerson': {
        description: 'Load canonical Person identity independently of Property context.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadPerson(personId, context),
      },
      'formLens.loadPropertyContext': {
        description: 'Load canonical Person-to-Property context independently of Person detail.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadPropertyContext(personId, context),
      },
      'formLens.selectProperty': {
        description: 'Choose the Property feeding the Listing Agreement draft.',
        execution: 'parallel',
        handle: async ({ propertyId }, context) => {
          context.update((model) => this.withProjection(model, propertyId))
        },
      },
      'formLens.fieldChanged': {
        description: 'Edit one local form field without mutating Person, Property, or persisted Forms.',
        execution: 'parallel',
        handle: async ({ name, value }, context) => {
          context.update((model) => ({
            ...model,
            fields: model.fields.map((field) =>
              field.name === name ? { ...field, value, origin: 'manual' } : field,
            ),
            manualFields: model.manualFields.includes(name)
              ? model.manualFields
              : [...model.manualFields, name],
          }))
        },
      },
      'formLens.resetDraft': {
        description: 'Discard local field edits and rehydrate from Person, Property, and template defaults.',
        execution: 'serial',
        handle: async (_request, context) => {
          context.update((model) => this.withProjection(
            { ...model, manualFields: [] },
            model.selectedPropertyId,
            true,
          ))
        },
      },
    }
  }

  override dispose(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = null
    super.dispose()
  }

  private withProjection(
    model: FormLensPageModel,
    requestedPropertyId: string | null = model.selectedPropertyId,
    resetManual = false,
  ): FormLensPageModel {
    const projection = projectListingAgreement(
      this.template,
      model.client,
      model.propertyContext,
      requestedPropertyId,
    )
    const manualFields = resetManual ? [] : model.manualFields
    const manual = new Set(manualFields)
    const currentValues = new Map(model.fields.map((field) => [field.name, field.value]))
    const fields = projection.fields.map((field) =>
      manual.has(field.name)
        ? { ...field, value: currentValues.get(field.name) ?? field.value, origin: 'manual' as const }
        : field,
    )

    return {
      ...model,
      selectedPropertyId: projection.selectedPropertyId,
      fields,
      manualFields,
    }
  }

  private scheduleSearchLoad(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer)
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null
      void this.dispatch({ operation: 'formLens.load', payload: {} })
    }, 250)
  }

  private async loadList(context: PageOperationContext<FormLensPageModel>): Promise<void> {
    const { query, page, pageSize, selectedPersonId } = context.snapshot()
    context.update((model) => ({ ...model, listLoading: true, listError: null }))

    try {
      const result = await this.source.loadList(
        { query, page, pageSize },
        { signal: context.signal },
      )
      const pageCount = Math.max(1, Math.ceil(result.total / result.pageSize))
      const selectedStillVisible = selectedPersonId
        ? result.rows.some((row) => row.id === selectedPersonId)
        : false
      const nextSelectedId = selectedStillVisible
        ? selectedPersonId
        : result.rows[0]?.id ?? null

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
        void this.dispatch({
          operation: 'formLens.selectPerson',
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

  private async loadPerson(
    personId: string,
    context: PageOperationContext<FormLensPageModel>,
  ): Promise<void> {
    try {
      const client = await this.source.loadPerson(personId, { signal: context.signal })
      if (!context.isCurrent() || context.snapshot().selectedPersonId !== personId) return
      context.update((model) => this.withProjection({
        ...model,
        client,
        clientLoading: false,
        clientError: client ? null : 'Person not found.',
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => this.withProjection({
        ...model,
        client: null,
        clientLoading: false,
        clientError: message,
      }))
    }
  }

  private async loadPropertyContext(
    personId: string,
    context: PageOperationContext<FormLensPageModel>,
  ): Promise<void> {
    try {
      const propertyContext = await this.source.loadPropertyContext(personId, {
        signal: context.signal,
      })
      if (!context.isCurrent() || context.snapshot().selectedPersonId !== personId) return
      context.update((model) => this.withProjection({
        ...model,
        propertyContext,
        propertyLoading: false,
        propertyError: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => this.withProjection({
        ...model,
        propertyContext: null,
        propertyLoading: false,
        propertyError: message,
      }))
    }
  }
}
