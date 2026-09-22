import { NextResponse, type NextRequest } from 'next/server'

import { getMarketingContent } from '@/legacy/db/marketing-content'
import { buildHomeContent } from '@/lib/marketing-content'
import { formatArea, formatPrice, propertyLocation } from '@/lib/property'
import { getProperties } from '@/lib/property-reads'
import type { PropertySummary } from '@/legacy/services/property'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// PAGE CONTENT FOR THE RUST UI ON THE PUBLIC SITE.
//
// WHY THIS IS NOT THE ROWS ROUTE. A list screen's data is a list and the rows route answers it. A page is not a list:
// its hero has an image and an alt text, its sections have eyebrows and calls to action, and the shape of that content
// is what lets Rust lay the page out instead of printing it. Serving a page as rows is how a converted homepage ends
// up as a heading and three lines of text with none of its design - which is exactly what happened, and why this route
// exists rather than another case in the rows one.
//
// UNAUTHENTICATED BY DESIGN, and constrained the same way the rows route is: it reads only what this site already
// publishes to anonymous visitors, through the same read models the live pages read, with `publicOnly: true` so the
// working lifecycle set cannot leak.
//
// THE DECISION ABOUT WHAT IS MISSING IS MADE HERE, ONCE. The live page degraded per source - a failed properties read
// never took the content with it - and that behaviour is kept: each read returns its own result and a failure empties
// only its own part of the payload.
// ---------------------------------------------------------------------------

/** A marketing block, in the shape the Rust `Block` expects: camelCase, and every optional field present as null. */
function block(source: unknown) {
  const b = (source ?? {}) as Record<string, unknown>
  const text = (value: unknown) => (typeof value === 'string' ? value : null)
  return {
    eyebrow: text(b.eyebrow) ?? '',
    title: text(b.title) ?? '',
    subtitle: text(b.subtitle) ?? '',
    body: text(b.body) ?? '',
    ctaLabel: text(b.ctaLabel),
    ctaHref: text(b.ctaHref),
    imagePath: text(b.imagePath),
    imageAlt: text(b.imageAlt),
    items: Array.isArray(b.items)
      ? (b.items as Record<string, unknown>[]).map((item) => ({
          key: text(item.key) ?? '',
          label: text(item.label),
          value: text(item.value),
        }))
      : [],
  }
}

/** A property, in the shape the Rust `Listing` expects.
 *
 * THE NAMES ARE THE READ MODEL'S, NOT GUESSED: `listPrice`, `heroUrl`, `bedrooms`. My first pass assumed `price`,
 * `imagePath` and `beds`, and every card came back with a null price and no photograph — the page would have rendered
 * four empty frames and a "Price upon request" on an estate with a price. Formatting goes through the same helpers the
 * TypeScript cards used, so a price reads the same on both sides.
 */
function listing(source: PropertySummary) {
  return {
    slug: source.slug ?? '',
    name: source.name ?? '',
    location: propertyLocation(source) ?? null,
    price: source.listPrice == null ? null : formatPrice(source.listPrice),
    kind: source.propertyType ?? null,
    imagePath: source.heroUrl ?? null,
    imageAlt: source.heroAlt ?? null,
    beds: source.bedrooms ?? null,
    baths: source.bathrooms ?? null,
    area: formatArea(source.lotSize, source.lotSizeUnits) ?? null,
    featured: source.featured === true,
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? ''

  switch (screen) {
    case 'site-home': {
      // The same two reads the live homepage made, at the same time, each with its own failure.
      const [propertiesResult, contentResult] = await Promise.all([
        getProperties({ publicOnly: true }),
        getMarketingContent(),
      ])
      const properties = propertiesResult.ok ? propertiesResult.data : []
      const home = contentResult.ok ? buildHomeContent(contentResult.data) : undefined
      return NextResponse.json({
        hero: block(home?.hero),
        buyers: block(home?.buyers),
        sellers: block(home?.sellers),
        culture: block(home?.culture),
        about: block(home?.about),
        contact: block(home?.contact),
        featured: properties.filter((property) => property.featured === true).map(listing),
        listings: properties.map(listing),
      })
    }
    default:
      // An empty payload rather than an error: a screen with no page content is a state Rust already renders.
      return NextResponse.json({})
  }
}

export const GET = withApiHandler(
  { label: '/api/rust-ui/public-page', route: '/api/rust-ui/public-page' },
  GETHandler,
)
