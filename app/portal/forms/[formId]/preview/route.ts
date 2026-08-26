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
import type { TemplateFieldValues, TemplateSectionValues } from '@/lib/forms/template-types'

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
  templateId: string,
  actorAppUserId: string,
  fieldValues: TemplateFieldValues,
) {
  const [people, issuedVersion] = await Promise.all([
    listFormSignerPeople(formId),
    getNextIssuedVersionForTemplate({ dealId, templateId }),
  ])
  const participants = canonicalizeExecutionParticipants(people)
  const template = getTemplate(templateId)
  if (!template) return { error: 'Template not found.' } as const
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
  const template = getTemplate(form.templateId)
  if (!template) return new NextResponse('Not found', { status: 404 })
  const renderContext = await previewRenderContext(
    formId,
    form.dealId,
    form.templateId,
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
  const template = getTemplate(form.templateId)
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
    form.templateId,
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
