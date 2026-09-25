import { NextRequest, NextResponse } from 'next/server'

import { getGuideItems } from '@/legacy/db/guide'
import { getMarketingContent } from '@/legacy/db/marketing-content'
// THE DETAIL READ GOES THROUGH THE SERVICE TOO. It used to reach the legacy SQL module directly; the service kernel
// (lib/property-reads → property.bySlug → the repository) exposes the same function, and using it here means the
// public site has one path to property data instead of two.
import { getPropertyBySlug } from '@/lib/property-reads'
import { rustApiPublicListings } from '@/lib/rust-api/client'
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
    throw new Error('a property record needs a slug, and none was given')
  }
  const result = await getPropertyBySlug(scope)
  if (!result.ok) throw new Error(`the property could not be read: ${result.error.kind}`)
  const record = result.data
  if (!record) return []

  const { property } = record
  // EVERY PROPERTY WE SELL IS ON CULEBRA, so a missing city is not a problem to report — it is the only answer there
  // is. Defaulting it removes a precondition that could never have failed for a real reason.
  const location =
    [property.city, property.stateOrProvince].filter(Boolean).join(', ') ||
    property.neighborhood ||
    'Culebra, PR'
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

async function contentRows(slot: string | null): Promise<RustUiRow[]> {
  const result = await getMarketingContent()
  if (!result.ok) throw new Error(`marketing content is unavailable: ${result.error.kind}`)
  // The block's own `id` is the slot key (`home.hero`, `home.services.buyers`, …) — that is how the live pages address
  // their copy. ASSUMPTION, stated rather than hidden: if a block id is not the slot string, this returns nothing and
  // the screen says there is nothing to show, instead of showing another page's words.
  const blocks = slot ? result.data.filter((block) => block.id === slot) : result.data
  return blocks.flatMap((block) => [
    { id: `${block.id}:title`, cells: [block.eyebrow ?? '—', block.title ?? '(untitled)'], badge: block.kind },
    ...(block.subtitle ? [{ id: `${block.id}:subtitle`, cells: ['Subtitle', block.subtitle] }] : []),
    ...(block.body ? [{ id: `${block.id}:body`, cells: ['Body', block.body] }] : []),
    ...(block.ctaLabel ? [{ id: `${block.id}:cta`, cells: ['Call to action', `${block.ctaLabel} → ${block.ctaHref ?? '—'}`] }] : []),
  ])
}

async function guideRows(): Promise<RustUiRow[]> {
  const items = await getGuideItems()
  return items.map((item, index) => ({
    id: `guide-${index}`,
    cells: Object.values(item as Record<string, unknown>)
      .filter((value) => value !== null && value !== undefined && typeof value !== 'object')
      .map((value) => String(value)),
  }))
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
    case 'site-guide':
      return NextResponse.json(await guideRows())
    default:
      return NextResponse.json([])
  }
}

export const GET = withApiHandler(
  { label: '/api/rust-ui/public-rows', route: '/api/rust-ui/public-rows' },
  GETHandler,
)
