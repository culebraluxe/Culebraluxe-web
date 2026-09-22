import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiCreateDeal, rustApiRead } from '@/lib/rust-api/client'

type DealPortfolioItem = {
  id: string
  propertyId: string
  propertyName: string
  propertyLocation: string
  propertyDescriptor: string | null
  heroMediaId: string | null
  clientId: string
  clientName: string
  stage: string
  listPrice: number | null
  offerPrice: number | null
  owner: string
  closingDate: string | null
  nextMilestone: string | null
  nextMilestoneAt: string | null
  lastActivity: string | null
  lastActivityAt: string | null
  showingCount: number
  offerCount: number
  participantCount: number
  latestOfferAmount: number | null
  latestOfferStatus: string | null
}

type DealContractPortfolioItem = {
  id: string
  formTemplateId: string
  contractType: string
  propertyId: string
  propertyLabel: string | null
  status: string
  processInstanceId: string | null
  executedAt: string | null
  createdAt: string
}

type DealableProperty = {
  id: string
  name: string
  location: string | null
}

type DealOwnerCandidate = {
  id: string
  displayName: string
  email: string | null
}

type DealPortfolioSnapshot = {
  deals: DealPortfolioItem[]
  contracts: DealContractPortfolioItem[]
  properties: DealableProperty[]
  users: DealOwnerCandidate[]
}

type PersonSearchResult = {
  id: string
  display_name: string
  role: string
  status: string
  location: string | null
  email: string | null
  phone: string | null
}

type CreateDealInput = {
  propertyId: string
  clientPersonId: string
  ownerUserId: string | null
  notes: string | null
}

function parseCreateDealInput(value: unknown): CreateDealInput | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const row = value as Record<string, unknown>

  const propertyId =
    typeof row.propertyId === 'string' ? row.propertyId.trim() : ''
  const clientPersonId =
    typeof row.clientPersonId === 'string' ? row.clientPersonId.trim() : ''

  if (!propertyId || !clientPersonId) return null
  if (row.ownerUserId != null && typeof row.ownerUserId !== 'string') return null
  if (row.notes != null && typeof row.notes !== 'string') return null

  const ownerUserId =
    typeof row.ownerUserId === 'string' && row.ownerUserId.trim()
      ? row.ownerUserId.trim()
      : null
  const notes =
    typeof row.notes === 'string' && row.notes.trim() ? row.notes.trim() : null

  return { propertyId, clientPersonId, ownerUserId, notes }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const peopleSearch = req.nextUrl.searchParams.get('peopleSearch')
  if (peopleSearch !== null) {
    const query = peopleSearch.trim()
    if (query.length < 2) {
      return NextResponse.json({ people: [] })
    }

    const params = new URLSearchParams({ query, limit: '20' })
    const result = await rustApiRead<PersonSearchResult[]>(
      ('/v1/people/search?' + params.toString()) as `/v1/${string}`,
    )

    return NextResponse.json({
      people: result.value.map((person) => ({
        id: person.id,
        displayName: person.display_name,
        role: person.role,
        status: person.status,
        location: person.location,
        email: person.email,
        phone: person.phone,
      })),
    })
  }

  const result = await rustApiRead<DealPortfolioSnapshot>('/v1/deals')
  return NextResponse.json({ deals: result.value })
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  let raw: unknown
  try {
    raw = await req.json()
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body.' }, { status: 400 })
  }

  const input = parseCreateDealInput(raw)
  if (!input) {
    return NextResponse.json(
      { error: 'propertyId and clientPersonId are required.' },
      { status: 400 },
    )
  }

  const created = await rustApiCreateDeal<{ id: string }>({
    propertyId: input.propertyId,
    clientPersonId: input.clientPersonId,
    ownerUserId: input.ownerUserId,
    notes: input.notes,
  })

  return NextResponse.json({ id: created.value.id })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/deals', route: '/api/portal/rust-ui/deals' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/deals', route: '/api/portal/rust-ui/deals' },
  POSTHandler,
)
