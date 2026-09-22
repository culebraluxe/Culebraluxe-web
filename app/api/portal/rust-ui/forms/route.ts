import { NextResponse, type NextRequest } from 'next/server'

import {
  rustApiCreateForm,
  rustApiRead,
  rustApiUpdateForm,
} from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'
import {
  emptyDealFacts,
  emptySectionValues,
  prefillFieldValues,
} from '@/lib/forms/offer-letter-data'
import {
  getActiveTemplate,
  getTemplate,
  listPortalFormTypes,
} from '@/lib/forms/template-registry'
import type { TemplateDefinition } from '@/lib/forms/template-types'

type FormInstance = {
  id: string
  templateId: string
  templateVersion: number
  dealId: string | null
  personId: string | null
  propertyId: string | null
  contractId: string | null
  status: string
  fieldValues: Record<string, string>
  sections: Record<string, string>
  createdByUserId: string | null
  createdAt: string
  updatedAt: string
}

type FormListItem = FormInstance & {
  dealLabel: string | null
  propertyLabel: string | null
  clientName: string | null
}

type DealFormFacts = {
  clientName: string | null
  propertyLabel: string | null
  offerAmount: string | null
  financingType: string | null
  closingDate: string | null
  personDisplayName: string | null
  propertyName: string | null
  propertyLocation: string | null
}

type FormSignerPerson = {
  personId: string | null
  name: string
  email: string | null
  role: string
}

type IssuedDocument = {
  documentId: string
  issuedVersion: number
  checksum: string
  createdAt: string
  mediaId: string | null
  sourceSnapshot?: unknown
}

function templatePayload(template: TemplateDefinition) {
  return {
    id: template.id,
    version: template.version,
    activeVersion: getActiveTemplate(template.id)?.version ?? template.version,
    displayName: template.displayName,
    documentTypeLabel: template.documentTypeLabel,
    renderingTitle: template.rendering.title,
    presentation: template.rendering.presentation,
    fields: template.fields.map((field) => ({
      name: field.name,
      label: field.label,
      type: field.type,
      required: field.required,
      options: [...(field.options ?? [])],
      when: field.when
        ? { field: field.when.field, values: [...field.when.values] }
        : null,
    })),
    sections: template.sections.map((section) => ({
      name: section.name,
      label: section.label,
      editable: section.editable,
      when: section.when
        ? { field: section.when.field, values: [...section.when.values] }
        : null,
    })),
  }
}

function itemPayload(item: FormListItem) {
  const template = getTemplate(item.templateId, item.templateVersion)
  return {
    id: item.id,
    templateId: item.templateId,
    templateVersion: item.templateVersion,
    templateName: template?.displayName ?? item.templateId,
    activeVersion: getActiveTemplate(item.templateId)?.version ?? item.templateVersion,
    status: item.status,
    dealId: item.dealId,
    personId: item.personId,
    propertyId: item.propertyId,
    contractId: item.contractId,
    dealLabel: item.dealLabel,
    propertyLabel: item.propertyLabel,
    clientName: item.clientName,
    fieldValues: item.fieldValues,
    sections: item.sections,
    updatedAt: item.updatedAt,
  }
}

async function recordPayload(formId: string) {
  const encoded = encodeURIComponent(formId)
  const [form, list, signers, issued] = await Promise.all([
    rustApiRead<FormInstance>(('/v1/forms/' + encoded) as `/v1/${string}`),
    rustApiRead<FormListItem[]>('/v1/forms'),
    rustApiRead<FormSignerPerson[]>(('/v1/forms/' + encoded + '/signers') as `/v1/${string}`),
    rustApiRead<IssuedDocument | null>(
      ('/v1/forms/' + encoded + '/issued-document') as `/v1/${string}`,
    ),
  ])
  const template = getTemplate(form.value.templateId, form.value.templateVersion)
  if (!template) {
    throw new Error(
      `Template ${form.value.templateId} v${form.value.templateVersion} is not registered.`,
    )
  }

  return {
    items: list.value.map(itemPayload),
    selected: {
      ...itemPayload({
        ...form.value,
        dealLabel: null,
        propertyLabel: null,
        clientName: null,
      }),
      createdAt: form.value.createdAt,
    },
    template: templatePayload(template),
    issued: issued.value,
    signers: signers.value,
    templateChoices: listPortalFormTypes().map((item) => ({
      id: item.id,
      displayName: item.displayName,
      activeVersion: getActiveTemplate(item.id)?.version ?? 1,
    })),
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? 'forms'
  const scope = req.nextUrl.searchParams.get('scope')

  if (screen === 'form-record') {
    if (!scope) {
      return NextResponse.json({ error: 'form-record requires scope.' }, { status: 400 })
    }
    return NextResponse.json({ forms: await recordPayload(scope) })
  }

  if (screen !== 'forms') {
    return NextResponse.json({ error: `unsupported forms screen '${screen}'` }, { status: 400 })
  }

  const list = await rustApiRead<FormListItem[]>('/v1/forms')
  return NextResponse.json({
    forms: {
      items: list.value.map(itemPayload),
      selected: null,
      template: null,
      issued: null,
      signers: [],
      templateChoices: listPortalFormTypes().map((item) => ({
        id: item.id,
        displayName: item.displayName,
        activeVersion: getActiveTemplate(item.id)?.version ?? 1,
      })),
    },
  })
}

type SaveRequest = {
  action: 'save'
  formId: string
  fieldValues: Record<string, string>
  sections: Record<string, string>
}

type CreateRequest = {
  action: 'create'
  templateId: string
  dealId?: string | null
  personId?: string | null
  propertyId?: string | null
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const input = (await req.json()) as SaveRequest | CreateRequest

  if (input.action === 'save') {
    if (!input.formId?.trim()) {
      return NextResponse.json({ error: 'formId is required.' }, { status: 400 })
    }
    await rustApiUpdateForm<FormInstance>(input.formId, {
      fieldValues: input.fieldValues,
      sections: input.sections,
    })
    return NextResponse.json({ forms: await recordPayload(input.formId) })
  }

  if (input.action === 'create') {
    const template = getActiveTemplate(input.templateId)
    if (!template) {
      return NextResponse.json({ error: 'Template not found.' }, { status: 400 })
    }

    const dealId = input.dealId?.trim() || null
    const personId = input.personId?.trim() || null
    const propertyId = input.propertyId?.trim() || null
    if (!dealId && !personId && !propertyId) {
      return NextResponse.json(
        { error: 'Select a deal, client, or property before creating a form.' },
        { status: 400 },
      )
    }

    let facts = emptyDealFacts()
    if (dealId) {
      const read = await rustApiRead<DealFormFacts | null>(
        ('/v1/forms/deal-facts/' + encodeURIComponent(dealId)) as `/v1/${string}`,
      )
      facts = read.value ?? facts
    }

    const created = await rustApiCreateForm<FormInstance>({
      templateId: template.id,
      templateVersion: template.version,
      dealId,
      personId,
      propertyId,
      fieldValues: prefillFieldValues(template, facts),
      sections: emptySectionValues(template),
    })
    return NextResponse.json({ formId: created.value.id })
  }

  return NextResponse.json({ error: 'Unsupported Forms action.' }, { status: 400 })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/forms', route: '/api/portal/rust-ui/forms' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/forms', route: '/api/portal/rust-ui/forms' },
  POSTHandler,
)
