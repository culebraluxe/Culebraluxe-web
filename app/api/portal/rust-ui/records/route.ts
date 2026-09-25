import { NextRequest, NextResponse } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead, rustApiUpdatePropertyAdmin } from '@/lib/rust-api/client'

const PAGE_SIZE = 50

type PropertySummary = {
  id: string
  name: string
  status: string
  location: string | null
  listPrice: string | null
  slug?: string | null
  archived: boolean
  imageCount: number
  videoCount: number
}

type PropertyPage = {
  rows: PropertySummary[]
  total: number
  page: number
  pageSize: number
}

type PropertyAdminRecord = PropertySummary & Record<string, unknown>

function money(amount: string | null): string | null {
  if (amount == null || amount.trim() === '') return null
  const numeric = Number(amount)
  if (!Number.isFinite(numeric)) return amount
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(numeric)
}

function mapRow(row: PropertySummary) {
  return {
    id: row.id,
    name: row.name,
    status: row.status,
    location: row.location ?? '',
    listPrice: money(row.listPrice),
    slug: row.slug ?? null,
    archived: row.archived,
    imageCount: row.imageCount,
    videoCount: row.videoCount,
  }
}

async function pagePayload(req: NextRequest) {
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
  const selectedId = selected ?? page.value.rows[0]?.id ?? null
  let selectedRow =
    page.value.rows.find((row) => row.id === selectedId) ?? null
  if (selectedId && !selectedRow) {
    try {
      selectedRow = (
        await rustApiRead<PropertyAdminRecord>(
          (`/v1/properties/${encodeURIComponent(selectedId)}/admin`) as `/v1/${string}`,
        )
      ).value
    } catch {
      selectedRow = null
    }
  }

  return {
    records: {
      rows: page.value.rows.map(mapRow),
      total: page.value.total,
      page: page.value.page,
      pageSize: page.value.pageSize,
      selectedId: selectedRow?.id ?? page.value.rows[0]?.id ?? null,
      selected: selectedRow ? mapRow(selectedRow) : null,
    },
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  return NextResponse.json(await pagePayload(req))
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const body = (await req.json().catch(() => null)) as {
    action?: string
    propertyId?: string
  } | null
  const action = body?.action
  const propertyId = body?.propertyId?.trim()
  if (!propertyId || (action !== 'archive' && action !== 'restore')) {
    return NextResponse.json(
      { error: 'action and propertyId are required.' },
      { status: 400 },
    )
  }

  const current = await rustApiRead<PropertyAdminRecord>(
    (`/v1/properties/${encodeURIComponent(propertyId)}/admin`) as `/v1/${string}`,
  )
  await rustApiUpdatePropertyAdmin<PropertyAdminRecord>(propertyId, {
    ...current.value,
    archived: action === 'archive',
  })

  const url = new URL(req.url)
  url.searchParams.set('selected', propertyId)
  return NextResponse.json(await pagePayload(new NextRequest(url, req)))
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/records', route: '/api/portal/rust-ui/records' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/records', route: '/api/portal/rust-ui/records' },
  POSTHandler,
)
