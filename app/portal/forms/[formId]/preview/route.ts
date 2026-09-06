import { NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { listFormSignerPeople } from '@/db/form-signer'
import { resolveBrokerSignatureForIssuance } from '@/db/broker-signature'
import { sql } from '@/db/client'
import { canonicalizeExecutionParticipants } from '@/lib/agreements/participants'
import { guardPortalRoute } from '@/lib/auth/portal-session'
import { getTemplate } from '@/lib/forms/template-registry'
import { renderFormPdf } from '@/lib/forms/pdf'
import type {
  TemplateDefinition,
  TemplateFieldValues,
  TemplateSectionValues,
} from '@/lib/forms/template-types'

export const dynamic = 'force-dynamic'

const LOCAL_BROKER_NAME = 'Lisa Penfield'

/** Template field that must identify Lisa before her local signature can compose. */
const LOCAL_BROKER_FIELD: Record<string, string> = {
  'LISTING-01': 'brokerName',
  'PR-PNS': 'sellerBrokerName',
  'PR-PNS-AMD': 'sellerBrokerName',
}

const LOCAL_BROKER_TEMPLATES = new Set(Object.keys(LOCAL_BROKER_FIELD))

function pdfResponse(bytes: Buffer) {
  return new NextResponse(new Uint8Array(bytes), {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': 'inline; filename="document.pdf"',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    },
  })
}

function previewValues(
  template: TemplateDefinition,
  values: TemplateFieldValues,
): TemplateFieldValues {
  const field = LOCAL_BROKER_FIELD[template.id]
  if (!field) return values
  return {
    ...values,
    [field]: LOCAL_BROKER_NAME,
  }
}

/**
 * Preview must work while Contract lineage migration 121 is still rolling out.
 * to_jsonb(row)->>'contract_id' safely reads as null when the physical column
 * does not exist, while preserving the same legacy Deal-lineage rule once it does.
 */
async function previewIssuedVersion(
  dealId: string | null,
  templateId: string,
): Promise<number> {
  const rows = await sql`
    select td.issued_version
    from transaction_document td
    where td.deal_id is not distinct from ${dealId}
      and to_jsonb(td)->>'contract_id' is null
      and td.template_id = ${templateId}
      and td.source = 'generated'
      and td.issued_version is not null
    order by td.issued_version desc, td.created_at desc
    limit 1
  `
  return Number((rows[0] as { issued_version?: unknown } | undefined)?.issued_version ?? 0) + 1
}

async function previewRenderContext(
  formId: string,
  dealId: string | null,
  template: TemplateDefinition,
  actorAppUserId: string,
  fieldValues: TemplateFieldValues,
) {
  const values = previewValues(template, fieldValues)
  const [people, issuedVersion] = await Promise.all([
    listFormSignerPeople(formId),
    previewIssuedVersion(dealId, template.id),
  ])
  const canonicalParticipants = canonicalizeExecutionParticipants(people)

  const participants =
    LOCAL_BROKER_TEMPLATES.has(template.id) &&
    !canonicalParticipants.some((participant) => participant.role === 'SELLER_BROKER')
      ? [
          ...canonicalParticipants,
          {
            slotId: 'SELLER_BROKER:1',
            role: 'SELLER_BROKER',
            personId: null,
            name: LOCAL_BROKER_NAME,
            email: null,
            required: true,
            order: canonicalParticipants.length + 1,
          },
        ]
      : canonicalParticipants

  const brokerSignature = await resolveBrokerSignatureForIssuance(
    {
      template,
      values,
      participants,
      actorAppUserId,
      issuedAt: new Date().toISOString(),
      requireExecutionSlot: false,
    },
    sql,
  )
  if (!brokerSignature.ok) {
    return { error: brokerSignature.message } as const
  }
  return {
    issuedVersion,
    participants,
    values,
    appliedSignatures: brokerSignature.signatures,
  }
}

export async function GET(
  _request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const guard = await guardPortalRoute('deal.read')
  if (!guard.ok) return new NextResponse(guard.error, { status: guard.status })
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  const template = getTemplate(form.templateId, form.templateVersion)
  if (!template) return new NextResponse('Not found', { status: 404 })
  const renderContext = await previewRenderContext(
    formId,
    form.dealId,
    template,
    guard.actor.appUserId,
    form.fieldValues,
  )
  if ('error' in renderContext) {
    return new NextResponse(renderContext.error, { status: 503 })
  }
  return pdfResponse(
    await renderFormPdf(
      template,
      renderContext.values,
      form.sections,
      renderContext.issuedVersion,
      {
        participants: renderContext.participants,
        appliedSignatures: renderContext.appliedSignatures,
      },
    ),
  )
}

export async function POST(
  request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const guard = await guardPortalRoute('deal.read')
  if (!guard.ok) return new NextResponse(guard.error, { status: guard.status })
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  const template = getTemplate(form.templateId, form.templateVersion)
  if (!template) return new NextResponse('Not found', { status: 404 })
  let fieldValues: TemplateFieldValues = form.fieldValues
  let sections: TemplateSectionValues = form.sections
  try {
    const body = (await request.json()) as {
      fieldValues?: TemplateFieldValues
      sections?: TemplateSectionValues
    }
    if (body.fieldValues && typeof body.fieldValues === 'object') {
      fieldValues = body.fieldValues
    }
    if (body.sections && typeof body.sections === 'object') {
      sections = body.sections
    }
  } catch {
    return new NextResponse('Invalid preview payload', { status: 400 })
  }
  const renderContext = await previewRenderContext(
    formId,
    form.dealId,
    template,
    guard.actor.appUserId,
    fieldValues,
  )
  if ('error' in renderContext) {
    return new NextResponse(renderContext.error, { status: 503 })
  }
  return pdfResponse(
    await renderFormPdf(
      template,
      renderContext.values,
      sections,
      renderContext.issuedVersion,
      {
        participants: renderContext.participants,
        appliedSignatures: renderContext.appliedSignatures,
      },
    ),
  )
}
