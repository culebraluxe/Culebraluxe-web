import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

const PAGE_SIZE = 50

type PropertyRow = {
  id: string
  name: string
  status: string
  slug?: string | null
  location?: string | null
  imageCount: number
}

type PropertyPage = {
  rows: PropertyRow[]
  total: number
  page: number
  pageSize: number
}

function mapRow(row: PropertyRow) {
  return {
    id: row.id,
    name: row.name,
    status: row.status,
    slug: row.slug ?? null,
    imageCount: row.imageCount,
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const search = req.nextUrl.searchParams.get('search')?.trim() ?? ''
  const pageIndex = Math.max(
    0,
    Number.parseInt(req.nextUrl.searchParams.get('page') ?? '0', 10) || 0,
  )
  const selected = req.nextUrl.searchParams.get('selected')?.trim() || null
  const params = new URLSearchParams({
    search,
    page: String(pageIndex + 1),
    pageSize: String(PAGE_SIZE),
  })
  const page = await rustApiRead<PropertyPage>(
    ('/v1/properties/admin?' + params.toString()) as `/v1/${string}`,
  )

  const selectedId =
    selected ??
    page.value.rows[0]?.id ??
    null
  let selectedRow = page.value.rows.find((row) => row.id === selectedId) ?? null
  if (selectedId && !selectedRow) {
    try {
      selectedRow = (
        await rustApiRead<PropertyRow>(
          (`/v1/properties/${encodeURIComponent(selectedId)}/admin`) as `/v1/${string}`,
        )
      ).value
    } catch {
      selectedRow = null
    }
  }

  return NextResponse.json({
    listingMedia: {
      properties: page.value.rows.map(mapRow),
      total: page.value.total,
      page: page.value.page,
      pageSize: page.value.pageSize,
      selectedId: selectedRow?.id ?? page.value.rows[0]?.id ?? null,
      selected: selectedRow ? mapRow(selectedRow) : null,
    },
  })
}

export const GET = withApiHandler(
  {
    label: '/api/portal/rust-ui/listing-media',
    route: '/api/portal/rust-ui/listing-media',
  },
  GETHandler,
)
