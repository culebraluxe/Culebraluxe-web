import { NextRequest, NextResponse } from 'next/server'

import { rustApiRelationshipAction } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

export const dynamic = 'force-dynamic'

async function POSTHandler(req: NextRequest) {
  let body: Record<string, unknown>
  try {
    body = await req.json()
  } catch {
    return NextResponse.json(
      { ok: false, code: 'invalid', message: 'Invalid JSON body' },
      { status: 400 },
    )
  }

  try {
    const result = await rustApiRelationshipAction<Record<string, unknown>>(body)
    return NextResponse.json(result.value)
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        code: 'unknown',
        message: error instanceof Error ? error.message : 'Something went wrong.',
      },
      { status: 400 },
    )
  }
}

export const POST = withApiHandler(
  {
    label: '/api/portal/relationship-evidence-review/actions',
    route: '/api/portal/relationship-evidence-review/actions',
  },
  POSTHandler,
)
