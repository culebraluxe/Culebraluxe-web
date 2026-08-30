import { getTemplate } from '@/lib/forms/template-registry'
import { renderFormPdf } from '@/lib/forms/pdf'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    const template = getTemplate('LISTING-01', 2)
    if (!template) {
      return Response.json({ ok: false, message: 'LISTING-01 v2 not found' }, { status: 404 })
    }
    const values = Object.fromEntries(template.fields.map((field) => [field.name, '']))
    const bytes = await renderFormPdf(template, values, {}, 1, { participants: [] })
    const raw = bytes.toString('latin1')
    const imageObjects = (raw.match(/\/Subtype\s*\/Image/g) ?? []).length
    return Response.json({
      ok: imageObjects > 0,
      bytes: bytes.length,
      imageObjects,
      templateId: template.id,
      templateVersion: template.version,
    }, { headers: { 'Cache-Control': 'no-store' } })
  } catch (error) {
    return Response.json({
      ok: false,
      message: error instanceof Error ? error.message : String(error),
    }, { status: 500, headers: { 'Cache-Control': 'no-store' } })
  }
}
