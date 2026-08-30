import { NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { listFormSignerPeople } from '@/db/form-signer'
import { getNextIssuedVersionForTemplate } from '@/db/issued-document'
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

async function previewRenderContext(
  formId: string,
  dealId: string | null,
  template: TemplateDefinition,
  actorAppUserId: string,
  fieldValues: TemplateFieldValues,
) {
  const [people, issuedVersion] = await Promise.all([
    listFormSignerPeople(formId),
    getNextIssuedVersionForTemplate({ dealId, templateId: template.id }),
  ])
  const canonicalParticipants = canonicalizeExecutionParticipants(people)

  // LISTING-01 uses Lisa's standing local pre-signature before the external
  // BoldSign path. Draft forms may not yet have a persisted SELLER_BROKER
  // signer row, but preview still needs to render the exact pre-signed document
  // the operator will issue. Materialize only that deterministic local broker
  // slot here; issuance remains responsible for snapshotting immutable slots.
  const participants =
    template.id === 'LISTING-01' &&
    !canonicalParticipants.some((participant) => participant.role === 'SELLER_BROKER') &&
    (fieldValues.brokerName ?? '').trim()
      ? [
          ...canonicalParticipants,
          {
            slotId: 'SELLER_BROKER:1',
            role: 'SELLER_BROKER',
            personId: null,
            name: (fieldValues.brokerName ?? '').trim(),
            email: null,
            required: true,
            order: canonicalParticipants.length + 1,
          },
        ]
      : canonicalParticipants

  const brokerSignature = await resolveBrokerSignatureForIssuance(
    {
      template,
      values: fieldValues,
      participants,
      actorAppUserId,
      issuedAt: new Date().toISOString(),
    },
    sql,
  )
  if (!brokerSignature.ok && brokerSignature.outcome !== 'unauthorized') {
    return { error: brokerSignature.message } as const
  }
  return {
    issuedVersion,
    participants,
    appliedSignatures: brokerSignature.ok ? brokerSignature.signatures : [],
  }
}

// Saved-draft preview (iframe fallback). Live typing uses POST below.
export async function GET(
  _request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const guard = await guardPortalRoute('deal.read')
  if (!guard.ok) return new NextResponse(guard.error, { status: guard.status })
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  // A saved form previews against its exact stored template version.
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
      form.fieldValues,
      form.sections,
      renderContext.issuedVersion,
      {
        participants: renderContext.participants,
        appliedSignatures: renderContext.appliedSignatures,
      },
    ),
  )
}

// Live preview of the values currently in the editor — does not persist.
export async function POST(
  request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const guard = await guardPortalRoute('deal.read')
  if (!guard.ok) return new NextResponse(guard.error, { status: guard.status })
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  // A saved form previews against its exact stored template version.
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
      fieldValues,
      sections,
      renderContext.issuedVersion,
      {
        participants: renderContext.participants,
        appliedSignatures: renderContext.appliedSignatures,
      },
    ),
  )
}
