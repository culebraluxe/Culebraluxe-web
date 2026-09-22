import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

type RelationshipActivity = {
  observedCommunicationCount: number
  twoWay: boolean
}

type ClientSummary = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  primaryEmail?: string | null
  primaryPhone?: string | null
  relationshipActivity: RelationshipActivity
}

type ClientsPageResult = {
  rows: ClientSummary[]
  total: number
  page: number
  pageSize: number
}

type ClientDetail = {
  id: string
  displayName: string
  role: string
  status: string
  email?: string | null
  phone?: string | null
  budgetMin?: number | null
  budgetMax?: number | null
  timeline?: string | null
  assignedAgent?: string | null
  notes?: string | null
}

type CommsPanel = {
  personId: string
  aggregate: {
    observedCount: number
    inboundCount: number
    outboundCount: number
    twoWay: boolean
    firstObservedAt?: string | null
    lastInboundAt?: string | null
    lastOutboundAt?: string | null
    lastContactAt?: string | null
    lastContactLabel?: string | null
    activeSourceCount: number
    sourceCount: number
  }
  sources: Array<{
    source: string
    channel: string
    label: string
    totalCount: number
    twoWay: boolean
    lastContext?: string | null
    lastContactAt?: string | null
  }>
  moments: Array<{
    id: string
    channel?: string | null
    direction?: string | null
    occurredAt: string
    title?: string | null
    summary?: string | null
  }>
  momentCount: number
}

type PropertyAddress = {
  address_line1?: string | null
  city?: string | null
  state_or_province?: string | null
  neighborhood?: string | null
  postal_code?: string | null
  country?: string | null
}

type PersonPropertyContext = {
  person_id: string
  properties: Array<{
    relation: string
    relation_status?: string | null
    property: {
      id: string
      display_name: string
      address: PropertyAddress
    }
  }>
}

function addressLabel(address: PropertyAddress): string {
  return [
    address.address_line1,
    address.neighborhood,
    address.city,
    [address.state_or_province, address.postal_code].filter(Boolean).join(' ') || null,
    address.country,
  ]
    .filter((value): value is string => Boolean(value?.trim()))
    .join(', ')
}

async function hydrate(personId: string) {
  const encoded = encodeURIComponent(personId)
  const [detail, comms, properties] = await Promise.all([
    rustApiRead<ClientDetail | null>(('/v1/clients/' + encoded) as `/v1/${string}`),
    rustApiRead<CommsPanel>(('/v1/comms/' + encoded + '/panel?momentLimit=20') as `/v1/${string}`),
    rustApiRead<PersonPropertyContext>(('/v1/people/' + encoded + '/properties') as `/v1/${string}`),
  ])

  return {
    selected: detail.value,
    comms: comms.value,
    properties: properties.value.properties.map((entry) => ({
      id: entry.property.id,
      displayName: entry.property.display_name,
      relation: entry.relation,
      relationStatus: entry.relation_status ?? null,
      address: addressLabel(entry.property.address),
    })),
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? 'clients'

  if (screen === 'client-record') {
    const scope = req.nextUrl.searchParams.get('scope')
    if (!scope) {
      return NextResponse.json({ error: 'client-record requires scope.' }, { status: 400 })
    }
    const hydrated = await hydrate(scope)
    return NextResponse.json({
      clients: {
        rows: [],
        total: 0,
        page: 1,
        pageSize: 50,
        selectedId: scope,
        ...hydrated,
      },
    })
  }

  if (screen !== 'clients') {
    return NextResponse.json({ error: "unsupported client screen '" + screen + "'" }, { status: 400 })
  }

  const search = req.nextUrl.searchParams.get('search')?.trim() ?? ''
  const pageIndex = Math.max(
    0,
    Number.parseInt(req.nextUrl.searchParams.get('page') ?? '0', 10) || 0,
  )
  const selected = req.nextUrl.searchParams.get('selected')?.trim() || null
  const params = new URLSearchParams({
    search,
    sort: 'name',
    page: String(pageIndex + 1),
    pageSize: '50',
  })

  const directory = await rustApiRead<ClientsPageResult>(
    ('/v1/clients?' + params.toString()) as `/v1/${string}`,
  )
  const selectedId = selected ?? directory.value.rows[0]?.id ?? null
  const hydrated = selectedId
    ? await hydrate(selectedId)
    : { selected: null, comms: null, properties: [] }

  return NextResponse.json({
    clients: {
      rows: directory.value.rows.map((row) => ({
        id: row.id,
        displayName: row.displayName,
        nameResolved: row.nameResolved,
        role: row.role,
        status: row.status,
        primaryEmail: row.primaryEmail ?? null,
        primaryPhone: row.primaryPhone ?? null,
        observedCount: row.relationshipActivity.observedCommunicationCount,
        twoWay: row.relationshipActivity.twoWay,
      })),
      total: directory.value.total,
      page: directory.value.page,
      pageSize: directory.value.pageSize,
      selectedId,
      ...hydrated,
    },
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/clients', route: '/api/portal/rust-ui/clients' },
  GETHandler,
)
