import type { PnsCanonicalSnapshot, PnsFieldOrigin } from '@/lib/forms/pns-canonical-types'
import type { Client } from '@/lib/portal/types'
import type { TemplateDefinition, TemplateFieldDefinition } from '@/lib/forms/template-types'
import type { PersonPropertyContextDto } from '@/services/property'
import {
  auditPnsFieldBindings,
  getPnsFieldBinding,
  type PnsBindingAudit,
  type PnsFieldBinding,
  type PnsFieldOwner,
} from '@/lib/forms/pns-field-binding'

export type PnsLensOwnerFilter = 'all' | PnsFieldOwner | 'orphan'

export type PnsLensListItem = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  primaryEmail: string | null
  primaryPhone: string | null
}

export type PnsLensListPage = {
  rows: PnsLensListItem[]
  total: number
  page: number
  pageSize: number
}

export type PnsLensFieldModel = {
  definition: TemplateFieldDefinition
  binding: PnsFieldBinding | null
  value: string
  origin: PnsFieldOrigin
}

export type PnsLensModel = {
  templateId: string
  templateVersion: number
  fields: PnsLensFieldModel[]
  audit: PnsBindingAudit
  ownerFilter: PnsLensOwnerFilter

  query: string
  page: number
  pageSize: number
  pageCount: number
  total: number
  list: PnsLensListItem[]

  selectedPersonId: string | null
  selectedPropertyId: string | null
  client: Client | null
  propertyContext: PersonPropertyContextDto | null
  canonical: PnsCanonicalSnapshot | null
  manualFields: string[]

  listLoading: boolean
  contextLoading: boolean
  bindingLoading: boolean
  saving: boolean
  listError: string | null
  contextError: string | null
  bindingError: string | null
  saveStatus: string | null
}

type EmptyPayload = Record<string, never>

export type PnsLensIntentMap = {
  'pnsLens.load': { request: EmptyPayload; response: void }
  'pnsLens.queryChanged': { request: { query: string }; response: void }
  'pnsLens.previousPage': { request: EmptyPayload; response: void }
  'pnsLens.nextPage': { request: EmptyPayload; response: void }
  'pnsLens.selectPerson': { request: { personId: string }; response: void }
  'pnsLens.loadContext': { request: { personId: string }; response: void }
  'pnsLens.loadBinding': { request: { personId: string }; response: void }
  'pnsLens.selectProperty': { request: { propertyId: string }; response: void }
  'pnsLens.fieldChanged': { request: { field: string; value: string }; response: void }
  'pnsLens.ownerChanged': { request: { owner: PnsLensOwnerFilter }; response: void }
  'pnsLens.reset': { request: EmptyPayload; response: void }
  'pnsLens.save': { request: EmptyPayload; response: void }
}

export function buildPnsLensModel(template: TemplateDefinition): PnsLensModel {
  return {
    templateId: template.id,
    templateVersion: template.version,
    fields: template.fields.map((definition) => ({
      definition,
      binding: getPnsFieldBinding(definition.name),
      value: '',
      origin: 'empty',
    })),
    audit: auditPnsFieldBindings(template),
    ownerFilter: 'all',

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
    canonical: null,
    manualFields: [],

    listLoading: true,
    contextLoading: false,
    bindingLoading: false,
    saving: false,
    listError: null,
    contextError: null,
    bindingError: null,
    saveStatus: null,
  }
}
