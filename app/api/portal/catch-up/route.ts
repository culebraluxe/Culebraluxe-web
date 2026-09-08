import type { NextRequest } from 'next/server'

import { getCatchUpEligiblePage } from '@/db/catch-up'
import { withApiHandler } from '@/lib/error-capture-seam'
import { buildCatchUpQueue } from '@/lib/catchup/queue'

export const dynamic = 'force-dynamic'

// CATCH-UP queue — server-side paged, bounded (ENG-34). Returns only eligible
// people with a deterministic, explainable reason.
export const GET = withApiHandler(
  { label: 'api:catch-up', route: '/api/portal/catch-up' },
  async (request: NextRequest) => {
    const sp = request.nextUrl.searchParams
    const search = sp.get('search') ?? ''
    const page = Math.max(1, Number(sp.get('page') ?? '1') || 1)
    const pageSize = Math.max(1, Number(sp.get('pageSize') ?? '50') || 50)

    const { rows, total } = await getCatchUpEligiblePage({
      search,
      page,
      pageSize,
    })
    const items = buildCatchUpQueue(rows)

    return Response.json({ items, total, page, pageSize })
  },
)
