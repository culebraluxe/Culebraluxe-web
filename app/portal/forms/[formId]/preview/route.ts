import { NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { getTemplate } from '@/lib/forms/template-registry'
import { renderFormPdf } from '@/lib/forms/pdf'
import type { TemplateFieldValues, TemplateSectionValues } from '@/lib/forms/template-types'

export const dynamic = 'force-dynamic'

function pdfResponse(bytes: Buffer) {
  return new NextResponse(bytes, {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': 'inline; filename="preview.pdf"',
      'Cache-Control': 'no-store',
    },
  })
}

// Saved-draft preview (iframe fallback). Live typing uses POST below.
export async function GET(
  _request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  const template = getTemplate(form.templateId)
  if (!template) return new NextResponse('Not found', { status: 404 })
  return pdfResponse(
    renderFormPdf(
      template,
      form.fieldValues,
      form.sections,
      form.status === 'issued' ? 1 : 0,
    ),
  )
}

// Live preview of the values currently in the editor — does not persist.
export async function POST(
  request: Request,
  context: { params: Promise<{ formId: string }> },
) {
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
  return pdfResponse(renderFormPdf(template, fieldValues, sections, 0))
}
