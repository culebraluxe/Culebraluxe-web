import {
  LISTING_CANONICAL_FIELD_NAMES,
  type ListingCanonicalFieldName,
  type ListingCanonicalFields,
  type ListingFieldOrigin,
} from '@/lib/forms/listing-field-binding'
import type { TemplateDefinition } from '@/lib/forms/template-types'
import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '../runtime'
import { projectListingAgreement } from './listing-projection'
import {
  INITIAL_FORM_LENS_MODEL,
  type FormLensFieldOrigin,
  type FormLensIntentMap,
  type FormLensPageModel,
} from './model'
import type { FormLensSource } from './source'

const CANONICAL_FIELDS = new Set<string>(LISTING_CANONICAL_FIELD_NAMES)
const PHYSICAL_FIELDS = new Set<string>([
  'property',
  'propertyLocation',
  'legalOwnerName',
  'catastroNumber',
])

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

function bindingOrigin(
  fieldName: string,
  origin: ListingFieldOrigin,
): FormLensFieldOrigin {
  if (origin === 'person') return 'person'
  if (origin === 'listing_form') return 'listing_form'
  if (origin === 'property') {
    return fieldName === 'sellerResidenceAddress' ? 'property_relation' : 'property'
  }
  return 'unresolved'
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
      listingBinding: initialModel.listingBinding
        ? {
            ...initialModel.listingBinding,
            fields: { ...initialModel.listingBinding.fields },
            origins: { ...initialModel.listingBinding.origins },
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
        description: 'Select seller, then fan out Person, Property, and Listing-evidence hydration lanes.',
        execution: 'parallel',
        handle: async ({ personId }, context) => {
          context.update((model) => this.withProjection({
            ...model,
            selectedPersonId: personId,
            selectedPropertyId: null,
            client: null,
            propertyContext: null,
            listingBinding: null,
            manualFields: [],
            clientLoading: true,
            propertyLoading: true,
            bindingLoading: true,
            savingCanonical: false,
            clientError: null,
            propertyError: null,
            bindingError: null,
            canonicalStatus: null,
          }, null, true))
          void this.dispatch({ operation: 'formLens.loadPerson', payload: { personId } })
          void this.dispatch({ operation: 'formLens.loadPropertyContext', payload: { personId } })
          void this.dispatch({ operation: 'formLens.loadListingBinding', payload: { personId } })
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
      'formLens.loadListingBinding': {
        description: 'Load canonical-first LISTING-01 fields with the latest Listing draft as fallback evidence.',
        execution: 'latest',
        handle: async ({ personId }, context) => this.loadListingBinding(personId, context),
      },
      'formLens.selectProperty': {
        description: 'Choose the Property feeding the Listing Agreement draft.',
        execution: 'parallel',
        handle: async ({ propertyId }, context) => {
          context.update((model) => this.withProjection(model, propertyId))
        },
      },
      'formLens.fieldChanged': {
        description: 'Edit one local form field; canonical mutation remains explicit.',
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
            canonicalStatus: null,
          }))
        },
      },
      'formLens.resetDraft': {
        description: 'Discard local edits and rehydrate from canonical truth, Listing evidence, and template defaults.',
        execution: 'serial',
        handle: async (_request, context) => {
          context.update((model) => this.withProjection(
            { ...model, manualFields: [], canonicalStatus: null },
            model.selectedPropertyId,
            true,
          ))
        },
      },
      'formLens.promoteCanonical': {
        description: 'Promote only the six canonical-bound Listing fields back through Person/Property contracts.',
        execution: 'serial',
        handle: async (_request, context) => this.promoteCanonical(context),
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
    const binding = model.listingBinding
    const selectedPropertyId = projection.selectedPropertyId

    const fields = projection.fields.map((field) => {
      if (manual.has(field.name)) {
        return { ...field, value: currentValues.get(field.name) ?? field.value, origin: 'manual' as const }
      }

      if (binding && CANONICAL_FIELDS.has(field.name)) {
        const name = field.name as ListingCanonicalFieldName
        const physicalMismatch = PHYSICAL_FIELDS.has(name)
          && binding.physicalPropertyId
          && selectedPropertyId
          && binding.physicalPropertyId !== selectedPropertyId
        const value = binding.fields[name]
        if (!physicalMismatch && value) {
          return { ...field, value, origin: bindingOrigin(name, binding.origins[name]) }
        }
      }

      return field
    })

    return {
      ...model,
      selectedPropertyId,
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
        void this.dispatch({ operation: 'formLens.selectPerson', payload: { personId: nextSelectedId } })
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
      const propertyContext = await this.source.loadPropertyContext(personId, { signal: context.signal })
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

  private async loadListingBinding(
    personId: string,
    context: PageOperationContext<FormLensPageModel>,
  ): Promise<void> {
    try {
      const listingBinding = await this.source.loadListingBinding(personId, { signal: context.signal })
      if (!context.isCurrent() || context.snapshot().selectedPersonId !== personId) return
      context.update((model) => this.withProjection({
        ...model,
        listingBinding,
        bindingLoading: false,
        bindingError: null,
      }))
    } catch (error) {
      if (isAbortError(error) || context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => this.withProjection({
        ...model,
        listingBinding: null,
        bindingLoading: false,
        bindingError: message,
      }))
    }
  }

  private async promoteCanonical(
    context: PageOperationContext<FormLensPageModel>,
  ): Promise<void> {
    const model = context.snapshot()
    const personId = model.selectedPersonId
    if (!personId) return

    const fieldMap = new Map(model.fields.map((field) => [field.name, field.value]))
    const fields = Object.fromEntries(
      LISTING_CANONICAL_FIELD_NAMES.map((name) => [name, fieldMap.get(name) ?? '']),
    ) as ListingCanonicalFields

    context.update((current) => ({
      ...current,
      savingCanonical: true,
      bindingError: null,
      canonicalStatus: 'Promoting reviewed Listing facts…',
    }))

    try {
      const listingBinding = await this.source.saveListingBinding(
        personId,
        fields,
        model.selectedPropertyId ?? undefined,
      )
      context.update((current) => this.withProjection({
        ...current,
        listingBinding,
        manualFields: current.manualFields.filter((name) => !CANONICAL_FIELDS.has(name)),
        savingCanonical: false,
        bindingError: null,
        canonicalStatus: 'Canonical Person / Property updated.',
      }, current.selectedPropertyId))

      // Refresh the independent canonical lanes so the sidecar immediately shows
      // the same truth that the next form will consume.
      void this.dispatch({ operation: 'formLens.loadPerson', payload: { personId } })
      void this.dispatch({ operation: 'formLens.loadPropertyContext', payload: { personId } })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      context.update((current) => ({
        ...current,
        savingCanonical: false,
        bindingError: message,
        canonicalStatus: null,
      }))
    }
  }
}
