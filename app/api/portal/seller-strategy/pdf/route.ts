import { NextResponse, type NextRequest } from 'next/server'

import { buildDecisionPdf } from '@/lib/decision-analysis/pdf-core'
import type { Inputs, ModelResult } from '@/lib/decision-analysis/types'
import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'

type PdfPayload = {
  inputs: Inputs
  model: ModelResult
}

function filename(value: string): string {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
  return `${slug || 'seller-strategy'}.pdf`
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const actor = await getPortalActingUser()
  if (!actor) return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })

  const form = await req.formData()
  const raw = form.get('payload')
  if (typeof raw !== 'string' || !raw.trim()) {
    return NextResponse.json({ error: 'Seller Strategy payload is required.' }, { status: 400 })
  }

  let payload: PdfPayload
  try {
    payload = JSON.parse(raw) as PdfPayload
  } catch {
    return NextResponse.json({ error: 'Seller Strategy payload is invalid.' }, { status: 400 })
  }

  const bytes = await buildDecisionPdf(payload.inputs, payload.model)
  return new Response(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer, {
    status: 200,
    headers: {
      'content-type': 'application/pdf',
      'content-disposition': `attachment; filename="${filename(payload.inputs.propertyName)}"`,
      'cache-control': 'no-store',
    },
  })
}

export const POST = withApiHandler(
  {
    label: '/api/portal/seller-strategy/pdf',
    route: '/api/portal/seller-strategy/pdf',
  },
  POSTHandler,
)
