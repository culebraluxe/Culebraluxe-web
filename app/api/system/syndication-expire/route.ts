import { NextResponse, type NextRequest } from 'next/server'

import { expireStalePlacements } from '@/db/syndication-expire'

// ---------------------------------------------------------------------------
// Marketing — daily expiry sweep for Clasificados / Facebook placements.
//
// SCHEDULE ENTRY POINT for a Vercel cron (or any authenticated external
// scheduler). Fails closed unless the SYNDICATION_EXPIRE_KEY env var is set and
// sent on the x-syndication-key header. Schedule config is a deployment concern,
// not code — see docs/marketing-overnight-notes.md.
// ---------------------------------------------------------------------------

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const EXPIRE_KEY_HEADER = 'x-syndication-key'

export async function POST(request: NextRequest) {
  const expected = process.env.SYNDICATION_EXPIRE_KEY
  if (!expected) {
    return NextResponse.json(
      { ok: false, error: 'Syndication expiry is not configured (missing SYNDICATION_EXPIRE_KEY).' },
      { status: 503 },
    )
  }
  const provided = request.headers.get(EXPIRE_KEY_HEADER)
  if (provided !== expected) {
    return NextResponse.json({ ok: false, error: 'Unauthorized.' }, { status: 401 })
  }
  const result = await expireStalePlacements()
  return NextResponse.json({ ok: true, expired: result.expired }, { status: 200 })
}
