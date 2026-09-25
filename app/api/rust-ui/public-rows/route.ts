import { NextRequest, NextResponse } from 'next/server'

// THE PUBLIC PROPERTY READS COME FROM RUST. The grid and the record page used to reach the TypeScript kernel for this
// data; both go through the public service now — one door, one rule, for either surface.
import {
  rustApiPublicListings,
  rustApiPublicMarketingContent,
  rustApiPublicProperty,
} from '@/lib/rust-api/client'
import { MARKETING_SLOTS } from '@/lib/marketing-content'
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
  // THE INVENTORY COMES FROM THE RUST SERVICE. This route used to read Postgres itself through the legacy TS db
  // layer — the shortcut the service kernel calls "a surface that missed the service-layer refactor", and the reason
  // the public visibility rule had to be written twice. It is now a thin proxy: ask Rust, shape the rows for the
  // screen, and nothing else. The shape (id, cells, badge) is unchanged, so the grid renders exactly as before.
  const listings = await rustApiPublicListings()
  return listings.map((listing) => ({
    // The URL key: the slug when the listing has one, its id when it does not. Nothing is dropped for want of a URL.
    id: listing.key,
    cells: [
      listing.name,
      listing.propertyType ?? '—',
      // A price is a number. Blank is zero.
      money(listing.listPrice ?? 0),
      listing.featured ? 'Featured' : '—',
    ],
    badge: listing.status,
  }))
}

/**
 * One property, as the facts a visitor asks for, in the order they ask for them. Only fields the read model actually
 * returns are shown, and a missing fact is omitted rather than printed as a dash: on a record page, five real facts
 * read better than fifteen with eleven unknowns.
 */
async function recordRows(scope: string | null): Promise<RustUiRow[]> {
  if (!scope) {
    throw new Error('a property record needs a key, and none was given')
  }
  // FROM THE RUST SERVICE. This used to go through the TypeScript kernel for the same facts; now the property page
  // reads the same way the grid does. A key that resolves to nothing is an empty page, not an error.
  const property = await rustApiPublicProperty(scope)
  if (!property) return []

  // EVERY PROPERTY WE SELL IS ON CULEBRA, so a missing city is the only answer there is, not a gap to report.
  const location =
    [property.city, property.stateOrProvince].filter(Boolean).join(', ') ||
    property.neighborhood ||
    'Culebra, PR'
  const bedsBaths = [
    property.bedrooms ? `${property.bedrooms} bed` : null,
    property.bathrooms ? `${property.bathrooms} bath` : null,
  ]
    .filter(Boolean)
    .join(' · ')

  return [
    // A price is a number. Blank is zero.
    facts('Price', money(property.listPrice ?? 0)),
    facts('Status', property.status),
    facts('Type', property.propertyType),
    facts('Location', location),
    facts('Bedrooms and baths', bedsBaths),
    facts('Living area', property.squareFeet ? `${property.squareFeet} sq ft` : null),
    facts('Lot', property.lotSize ? `${property.lotSize} ${property.lotSizeUnits ?? ''}`.trim() : null),
    facts('Year built', property.yearBuilt),
    facts('Architecture', property.architectureNotes),
    // The hero is the marked photograph or the first one, so this is only ever "missing" for a Property with no
    // photographs at all.
    facts('Hero image', property.heroMediaId ? 'present' : 'missing'),
    facts(
      'Gallery',
      property.media.filter((item) => item.mediaType === 'image' && item.role !== 'hero').length
        ? `${property.media.filter((item) => item.mediaType === 'image' && item.role !== 'hero').length} image(s)`
        : null,
    ),
    facts('Videos', property.videoCount ? `${property.videoCount} video(s)` : null),
    facts('Description', property.shortDescription ?? property.editorialDescription),
  ].filter((row): row is RustUiRow => row !== null)
}

async function contentRows(slot: string | null): Promise<RustUiRow[]> {
  const content = await rustApiPublicMarketingContent()
  const blocks = slot ? content.filter((block) => block.id === slot) : content
  return blocks.flatMap((block) => [
    { id: `${block.id}:title`, cells: [block.eyebrow ?? '—', block.title ?? '(untitled)'], badge: block.kind },
    ...(block.subtitle ? [{ id: `${block.id}:subtitle`, cells: ['Subtitle', block.subtitle] }] : []),
    ...(block.body ? [{ id: `${block.id}:body`, cells: ['Body', block.body] }] : []),
    ...(block.ctaLabel ? [{ id: `${block.id}:cta`, cells: ['Call to action', `${block.ctaLabel} → ${block.ctaHref ?? '—'}`] }] : []),
  ])
}


async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  const scope = req.nextUrl.searchParams.get('scope')

  switch (screen) {
    case 'site-properties':
      return NextResponse.json(await listingRows())
    case 'site-property-detail':
      return NextResponse.json(await recordRows(scope))
    // The editorial pages read the marketing content slots the live pages read, by slot key.
    case 'site-home':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.hero))
    case 'site-about':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.about))
    case 'site-buyers':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.buyers))
    case 'site-sellers':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.sellers))
    case 'site-faq':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.faqList))
    case 'site-contact':
      return NextResponse.json(await contentRows(MARKETING_SLOTS.contact))
    default:
      return NextResponse.json([])
  }
}

export const GET = withApiHandler(
  { label: '/api/rust-ui/public-rows', route: '/api/rust-ui/public-rows' },
  GETHandler,
)
