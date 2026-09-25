import { NextRequest, NextResponse } from 'next/server'

import type { ReviewState } from '@/lib/relationship-intel/contracts'
import { rustApiRead } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

const VALID_REVIEW_STATES: ReviewState[] = [
  'unresolved',
  'exact_linked',
  'review_required',
  'ambiguous',
  'unmatched',
  'rejected',
  'non_person',
  'deferred',
]

async function GETHandler(req: NextRequest) {
  const params = req.nextUrl.searchParams
  const rawState = params.get('reviewState')
  const reviewState: ReviewState | 'all' =
    rawState && VALID_REVIEW_STATES.includes(rawState as ReviewState)
      ? (rawState as ReviewState)
      : 'all'
  const search = params.get('search') ?? ''
  const limit = Math.max(
    1,
    Math.min(100, parseInt(params.get('limit') ?? '50', 10) || 50),
  )
  const offset = Math.max(
    0,
    parseInt(params.get('offset') ?? '0', 10) || 0,
  )
  const query = new URLSearchParams({
    reviewState,
    search,
    limit: String(limit),
    offset: String(offset),
  })
  const result = await rustApiRead<{ rows: unknown[]; total: number }>(
    (`/v1/relationship-evidence/review?${query.toString()}`) as `/v1/${string}`,
  )
  return NextResponse.json(result.value)
}

export const GET = withApiHandler(
  {
    label: '/api/portal/relationship-evidence-review',
    route: '/api/portal/relationship-evidence-review',
  },
  GETHandler,
)
