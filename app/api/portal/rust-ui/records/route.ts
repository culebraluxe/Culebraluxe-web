import { NextRequest, NextResponse } from 'next/server'

import {
  archivePropertyAction,
  restorePropertyAction,
} from '@/app/portal/actions'
import { withApiHandler } from '@/lib/error-capture-seam'
import { guardPortalRoute } from '@/lib/auth/portal-session'
import { getPropertyAdmin, type PropertyAdminRow } from '@/legacy/db/property-admin'

const PAGE_SIZE = 50

function money(amount: number | null): string | null {
  if (amount == null) return null
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(amount)
}

function mapRow(row: PropertyAdminRow) {
  return {
    id: row.id,
    name: row.name,
    status: row.status,
    location: row.location ?? '',
    listPrice: money(row.listPrice),
    slug: row.slug,
    archived: row.archived,
    imageCount: row.imageCount,
    videoCount: row.videoCount,
  }
}

function pagePayload(req: NextRequest, rows: PropertyAdminRow[]) {
  const search = req.nextUrl.searchParams.get('search')?.trim().toLowerCase() ?? ''
  const pageIndex = Math.max(0, Number.parseInt(req.nextUrl.searchParams.get('page') ?? '0', 10) || 0)
  const selected = req.nextUrl.searchParams.get('selected')?.trim() || null
  const filtered = search
    ? rows.filter((row) =>
        [row.name, row.location, row.slug, row.status]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
          .includes(search),
      )
    : rows
  const total = filtered.length
  const pageRows = filtered.slice(pageIndex * PAGE_SIZE, pageIndex * PAGE_SIZE + PAGE_SIZE)
  const selectedId =
    (selected && pageRows.some((row) => row.id === selected) ? selected : null) ??
    pageRows[0]?.id ??
    null
  const selectedRow = rows.find((row) => row.id === selectedId) ?? null
  return {
    records: {
      rows: pageRows.map(mapRow),
      total,
      page: pageIndex + 1,
      pageSize: PAGE_SIZE,
      selectedId,
      selected: selectedRow ? mapRow(selectedRow) : null,
    },
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const guard = await guardPortalRoute('portal.read')
  if (!guard.ok) {
    return NextResponse.json({ error: guard.error }, { status: guard.status })
  }
  const rows = await getPropertyAdmin()
  return NextResponse.json(pagePayload(req, rows))
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const guard = await guardPortalRoute('listing.write')
  if (!guard.ok) {
    return NextResponse.json({ error: guard.error }, { status: guard.status })
  }
  const body = (await req.json().catch(() => null)) as {
    action?: string
    propertyId?: string
  } | null
  const action = body?.action
  const propertyId = body?.propertyId?.trim()
  if (!propertyId || (action !== 'archive' && action !== 'restore')) {
    return NextResponse.json({ error: 'action and propertyId are required.' }, { status: 400 })
  }
  const result =
    action === 'archive'
      ? await archivePropertyAction(propertyId)
      : await restorePropertyAction(propertyId)
  if (!result.ok) {
    return NextResponse.json({ error: result.message }, { status: 400 })
  }
  const rows = await getPropertyAdmin()
  const url = new URL(req.url)
  url.searchParams.set('selected', propertyId)
  const refresh = new NextRequest(url, req)
  return NextResponse.json(pagePayload(refresh, rows))
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/records', route: '/api/portal/rust-ui/records' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/records', route: '/api/portal/rust-ui/records' },
  POSTHandler,
)
