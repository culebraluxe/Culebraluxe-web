import { NextResponse } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

export const dynamic = 'force-dynamic'

async function GETHandler() {
  const result = await rustApiRead<Record<string, unknown>>(
    '/v1/diagnostics/db',
  )
  return NextResponse.json(result.value)
}

export const GET = withApiHandler(
  { label: '/api/portal/db-diag', route: '/api/portal/db-diag' },
  GETHandler,
)
