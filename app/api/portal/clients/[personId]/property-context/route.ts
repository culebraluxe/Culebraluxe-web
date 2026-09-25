import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

// Compatibility edge only. Property data and authorization are both owned by
// Rust; this route preserves the browser URL while the remaining Next API shell
// is being removed.
async function GETHandler(
  _request: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  const result = await rustApiRead<unknown>(
    (`/v1/people/${encodeURIComponent(personId)}/properties`) as `/v1/${string}`,
  )
  return NextResponse.json(result.value)
}

export const GET = withApiHandler(
  {
    label: '/api/portal/clients/[personId]/property-context',
    route: '/api/portal/clients/[personId]/property-context',
  },
  GETHandler,
)
