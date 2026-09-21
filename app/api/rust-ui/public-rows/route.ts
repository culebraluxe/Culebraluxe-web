import { NextRequest, NextResponse } from 'next/server'

import { getProperties, getPropertyBySlug } from '@/db/property-public-reads'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// ROWS FOR THE RUST UI ON THE PUBLIC SITE.
//
// THIS ROUTE IS UNAUTHENTICATED, BY DESIGN, AND THAT IS THE WHOLE CONSTRAINT ON IT: it may read only what this site
// already publishes to anonymous visitors. It reaches `property-public-reads` and nothing else — no person, no deal,
// no showing, no task, no accounting row, no trace. If a screen here ever needs a read model that is not public, that
// screen belongs behind the session in `/api/portal/rust-ui/rows`, not here.
//
// The publicOnly: true flag is not decorative: the read model's own contract says PUBLIC surfaces MUST pass it, and
// without it the working lifecycle set (including staged rows) comes back. An unauthenticated route that forgets it
// would publish inventory that is not published.
//
// Shape (`Row` in rust/ui/src/model.rs, camelCase on both sides):
//   { id: string, cells: string[], badge?: string | null }
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

type RustUiRow = { id: string; cells: string[]; badge?: string | null }

const money = (amount: number) =>
  new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 }).format(amount)

const facts = (label: string, value: string | number | null | undefined): RustUiRow | null =>
  value === null || value === undefined || value === '' ? null : { id: label, cells: [label, String(value)] }

async function listingRows(): Promise<RustUiRow[]> {
  const result = await getProperties({ publicOnly: true })
  if (!result.ok) throw new Error(`public inventory is unavailable: ${result.error.kind}`)
  return (
    result.data
      // A row with no slug cannot be opened, so it is not offered as one. Publishing a listing without a URL is a
      // data problem, and hiding it here would hide it forever.
      .filter((property) => Boolean(property.slug))
      .map((property) => ({
        id: property.slug as string,
        cells: [
          property.name,
          property.propertyType ?? '—',
          property.listPrice ? money(property.listPrice) : 'Price on request',
          property.featured ? 'Featured' : '—',
        ],
        badge: property.status,
      }))
  )
}

/**
 * One property, as the facts a visitor asks for, in the order they ask for them. Only fields the read model actually
 * returns are shown, and a missing fact is omitted rather than printed as a dash: on a record page, five real facts
 * read better than fifteen with eleven unknowns.
 */
async function recordRows(scope: string | null): Promise<RustUiRow[]> {
  if (!scope) {
    throw new Error('a property record needs a slug, and none was given')
  }
  const result = await getPropertyBySlug(scope)
  if (!result.ok) throw new Error(`the property could not be read: ${result.error.kind}`)
  const record = result.data
  if (!record) return []

  const { property } = record
  const location = [property.city, property.stateOrProvince].filter(Boolean).join(', ') || property.neighborhood
  const bedsBaths = [
    property.bedroomsTotal ? `${property.bedroomsTotal} bed` : null,
    property.bathroomsTotal ? `${property.bathroomsTotal} bath` : null,
  ]
    .filter(Boolean)
    .join(' · ')

  return [
    facts('Price', property.listPrice ? money(property.listPrice) : null),
    facts('Status', property.standardStatus),
    facts('Type', property.propertyType),
    facts('Location', location),
    facts('Bedrooms and baths', bedsBaths),
    facts('Living area', property.livingArea ? `${property.livingArea} sq ft` : null),
    facts('Lot', property.lotSizeArea ? `${property.lotSizeArea} ${property.lotSizeUnits ?? ''}`.trim() : null),
    facts('Year built', property.yearBuilt),
    facts('Architecture', property.architecture),
    facts('Hero image', record.heroUrl ? 'present' : 'missing'),
    facts('Gallery', record.galleryImages.length ? `${record.galleryImages.length} image(s)` : null),
    facts('Videos', record.videos.length ? `${record.videos.length} video(s)` : null),
    facts('Documents', record.documents.length ? `${record.documents.length} document(s)` : null),
    facts('Description', property.shortDescription ?? property.editorialDescription),
  ].filter((row): row is RustUiRow => row !== null)
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  const scope = req.nextUrl.searchParams.get('scope')

  switch (screen) {
    case 'site-properties':
      return NextResponse.json(await listingRows())
    case 'site-property-detail':
      return NextResponse.json(await recordRows(scope))
    default:
      // `site-home` is here: its content is editorial copy whose layout is not ported yet, and an empty list is an
      // honest answer where invented rows would not be.
      return NextResponse.json([])
  }
}

export const GET = withApiHandler(
  { label: '/api/rust-ui/public-rows', route: '/api/rust-ui/public-rows' },
  GETHandler,
)
