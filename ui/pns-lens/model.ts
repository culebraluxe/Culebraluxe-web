import type { TemplateDefinition, TemplateFieldDefinition } from '@/lib/forms/template-types'
import {
  auditPnsFieldBindings,
  getPnsFieldBinding,
  type PnsBindingAudit,
  type PnsFieldBinding,
  type PnsFieldOwner,
} from '@/lib/forms/pns-field-binding'

export type PnsLensOwnerFilter = 'all' | PnsFieldOwner | 'orphan'

export type PnsLensFieldModel = {
  definition: TemplateFieldDefinition
  binding: PnsFieldBinding | null
  value: string
}

export type PnsLensModel = {
  templateId: string
  templateVersion: number
  fields: readonly PnsLensFieldModel[]
  audit: PnsBindingAudit
  ownerFilter: PnsLensOwnerFilter
}

export type PnsLensIntentMap = {
  'pnsLens.fieldChanged': {
    request: { field: string; value: string }
    response: void
  }
  'pnsLens.ownerChanged': {
    request: { owner: PnsLensOwnerFilter }
    response: void
  }
  'pnsLens.reset': {
    request: Record<string, never>
    response: void
  }
}

export function buildPnsLensModel(template: TemplateDefinition): PnsLensModel {
  return {
    templateId: template.id,
    templateVersion: template.version,
    fields: template.fields.map((definition) => ({
      definition,
      binding: getPnsFieldBinding(definition.name),
      value: '',
    })),
    audit: auditPnsFieldBindings(template),
    ownerFilter: 'all',
  }
}
