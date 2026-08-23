import { NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { getTemplate } from '@/lib/forms/template-registry'
import { renderFormPdf } from '@/lib/forms/pdf'

export const dynamic = 'force-dynamic'

// Live preview uses the SAME renderer as issuance (lib/forms/pdf renderFormPdf).
export async function GET(
  _request: Request,
  context: { params: Promise<{ formId: string }> },
) {
  const { formId } = await context.params
  const form = await getFormInstance(formId)
  if (!form) return new NextResponse('Not found', { status: 404 })
  const template = getTemplate(form.templateId)
  if (!template) return new NextResponse('Not found', { status: 404 })
  const bytes = renderFormPdf(
    template,
    form.fieldValues,
    form.sections,
    form.status === 'issued' ? 1 : 0,
  )
  return new NextResponse(bytes, {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': 'inline; filename="preview.pdf"',
      'Cache-Control': 'no-store',
    },
  })
}
