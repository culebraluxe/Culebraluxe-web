import type { Client } from '@/lib/portal/types'
import type { TemplateFieldType } from '@/lib/forms/template-types'
import type { PersonPropertyContextDto } from '@/services/property'
import type { PageIntentMap } from '../runtime'

export type FormLensListItem = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  primaryEmail: string | null
  primaryPhone: string | null
}

export type FormLensListPage = {
  rows: FormLensListItem[]
  total: number
  page: number
  pageSize: number
}

export type FormLensFieldOrigin =
  | 'person'
  | 'property'
  | 'property_relation'
  | 'template_default'
  | 'manual'
  | 'unresolved'

export type FormLensFieldModel = {
  name: string
  label: string
  type: TemplateFieldType
  required: boolean
  options: readonly string[]
  value: string
  origin: FormLensFieldOrigin
}

export type FormLensPageModel = {
  query: string
  page: number
  pageSize: number
  pageCount: number
  total: number
  list: FormLensListItem[]

  selectedPersonId: string | null
  selectedPropertyId: string | null
  client: Client | null
  propertyContext: PersonPropertyContextDto | null

  fields: FormLensFieldModel[]
  manualFields: string[]

  listLoading: boolean
  clientLoading: boolean
  propertyLoading: boolean
  listError: string | null
  clientError: string | null
  propertyError: string | null
}

export const INITIAL_FORM_LENS_MODEL: FormLensPageModel = {
  query: '',
  page: 1,
  pageSize: 50,
  pageCount: 1,
  total: 0,
  list: [],

  selectedPersonId: null,
  selectedPropertyId: null,
  client: null,
  propertyContext: null,

  fields: [],
  manualFields: [],

  listLoading: true,
  clientLoading: false,
  propertyLoading: false,
  listError: null,
  clientError: null,
  propertyError: null,
}

type EmptyPayload = Record<string, never>

export type FormLensIntentMap = {
  'formLens.load': { request: EmptyPayload; response: void }
  'formLens.queryChanged': { request: { query: string }; response: void }
  'formLens.previousPage': { request: EmptyPayload; response: void }
  'formLens.nextPage': { request: EmptyPayload; response: void }
  'formLens.selectPerson': { request: { personId: string }; response: void }
  'formLens.loadPerson': { request: { personId: string }; response: void }
  'formLens.loadPropertyContext': { request: { personId: string }; response: void }
  'formLens.selectProperty': { request: { propertyId: string }; response: void }
  'formLens.fieldChanged': { request: { name: string; value: string }; response: void }
  'formLens.resetDraft': { request: EmptyPayload; response: void }
} satisfies PageIntentMap
