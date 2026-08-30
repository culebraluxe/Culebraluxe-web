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

const LISTING_BROKER_NAME = 'Lisa Penfield'

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
  if (template.id !== 'LISTING-01') return values
  return {
    ...values,
    // CulebraLuxe is currently a single-broker model. Preview must never depend
    // on an old/blank draft retaining this invariant manually.
    brokerName: LISTING_BROKER_NAME,
  }
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
    getNextIssuedVersionForTemplate({ dealId, templateId: template.id }),
  ])
  const canonicalParticipants = canonicalizeExecutionParticipants(people)

  // Draft preview is not issuance. It may display Lisa's standing local
  // pre-signature before immutable participant slots have been snapshotted.
  // Issuance still requires the real SELLER_BROKER slot before BoldSign starts.
  const participants =
    template.id === 'LISTING-01' &&
    !canonicalParticipants.some((participant) => participant.role === 'SELLER_BROKER')
      ? [
          ...canonicalParticipants,
          {
            slotId: 'SELLER_BROKER:1',
            role: 'SELLER_BROKER',
            personId: null,
            name: LISTING_BROKER_NAME,
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
