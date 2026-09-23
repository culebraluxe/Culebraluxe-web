import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { getPropertyAdmin, type PropertyAdminRow } from '@/legacy/db/property-admin'

const PAGE_SIZE = 50

function mapRow(row: PropertyAdminRow) {
  return {
    id: row.id,
    name: row.name,
    status: row.status,
    slug: row.slug,
    imageCount: row.imageCount,
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const rows = await getPropertyAdmin()
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
    (selected && filtered.some((row) => row.id === selected) ? selected : null) ??
    pageRows[0]?.id ??
    null
  const selectedRow = rows.find((row) => row.id === selectedId) ?? null
  return NextResponse.json({
    listingMedia: {
      properties: pageRows.map(mapRow),
      total,
      page: pageIndex + 1,
      pageSize: PAGE_SIZE,
      selectedId,
      selected: selectedRow ? mapRow(selectedRow) : null,
    },
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/listing-media', route: '/api/portal/rust-ui/listing-media' },
  GETHandler,
)
