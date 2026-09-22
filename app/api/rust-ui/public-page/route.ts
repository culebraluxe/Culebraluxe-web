import { NextResponse, type NextRequest } from 'next/server'

import { getGuideItems } from '@/legacy/db/guide'
import { getMarketingContent } from '@/legacy/db/marketing-content'
import {
  blockById,
  buildContactPageContent,
  buildFaqPageContent,
  buildHomeContent,
  MARKETING_SLOTS,
} from '@/lib/marketing-content'
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
    // THE EDITORIAL PAGES, each fed by the slot the live page read. These do not need a new read model: the copy is
    // already in Neon and `buildHomeContent` addresses it by slot. The payload is deliberately PARTIAL — a screen sends
    // the blocks it renders and nothing else — which is safe because `PageContent` carries `default` and `block()` fills
    // an absent block with empty values rather than nulls. One missing block empties that block, never the page.
    case 'site-services': {
      const result = await getMarketingContent()
      const home = result.ok ? buildHomeContent(result.data) : undefined
      // `components/services.tsx` takes exactly these two blocks, and their slot names say why: home.services.buyers and
      // home.services.sellers are the Services page's copy, rendered on the homepage as a summary and here in full.
      return NextResponse.json({ buyers: block(home?.buyers), sellers: block(home?.sellers) })
    }
    case 'site-sellers': {
      const result = await getMarketingContent()
      const home = result.ok ? buildHomeContent(result.data) : undefined
      return NextResponse.json({ sellers: block(home?.sellers) })
    }
    case 'site-buyers': {
      // THE BUYERS PAGE IS INVENTORY, NOT COPY. It read the same public properties the homepage's grids read, through the
      // same read model and the same formatting. The marketing `buyers` block is the *homepage's* summary of buying, and
      // serving it here is how the site's most important page ended up with the wrong words on it — the same mistake
      // that put "For Buyers" on the Services page.
      const result = await getProperties({ publicOnly: true })
      const properties = result.ok ? result.data : []
      return NextResponse.json({
        listings: properties.map(listing),
        featured: properties.filter((property) => property.featured === true).map(listing),
      })
    }
    case 'site-about': {
      const result = await getMarketingContent()
      const home = result.ok ? buildHomeContent(result.data) : undefined
      return NextResponse.json({ about: block(home?.about) })
    }
    case 'site-faq': {
      const result = await getMarketingContent()
      const blocks = result.ok ? result.data : []
      const faq = buildFaqPageContent(blocks)
      return NextResponse.json({
        hero: block(faq.hero),
        // The accordion is served as the `faq.list` block itself, whose items are the question/answer pairs keyed `faq` —
        // the same list `faqEntries()` reads from. Served whole, so the renderer can see the CTA carried on the block
        // (`ctaHeading`, `ctaLabel`, `ctaHref`) without a second selector and a second shape.
        faq: block(blockById(blocks, MARKETING_SLOTS.faqList)),
      })
    }
    case 'site-contact': {
      const result = await getMarketingContent()
      const blocks = result.ok ? result.data : []
      // The contact page's own two slots, exactly the shape `ContactPageContent` documents.
      const contact = buildContactPageContent(blocks)
      return NextResponse.json({ hero: block(contact.hero), contact: block(contact.contact) })
    }
    case 'site-guide': {
      // THE GUIDE IS THE ONE PAGE WHOSE CONTENT IS NOT EDITORIAL COPY. It is a catalogue of places read from
      // `guide_item`, with its `card` image and its section ordering. It is served here rather than through the rows
      // route because a guide entry is a card with a photograph, not a line in a table — and serving it as rows is what
      // the rows route does today, which is why the guide page had nothing to draw.
      //
      // Served as `guide` rather than as a `Block`: a place has a photograph, a section and a description, and forcing
      // that into `cells` is the abstraction this whole route exists to avoid. `PageContent` ignores the field until the
      // Rust type grows one, which is the renderer's commit.
      const items = await getGuideItems()
      return NextResponse.json({
        guide: items.map((item) => ({
          slug: item.slug,
          section: item.section,
          name: item.name,
          eyebrow: item.eyebrow,
          subtitle: item.subtitle,
          area: item.area,
          description: item.description,
          note: item.note,
          address: item.address,
          phone: item.phone,
          websiteUrl: item.websiteUrl,
          imagePath: item.imageUrl,
          imageAlt: item.imageAlt,
        })),
      })
    }
    default: {
      // NO SILENT EMPTY PAGE. Every screen that asks this route for a page is an editorial screen — asking is what
      // makes it one — so a screen with no case here is a wiring mistake, not a page that happens to have no content.
      // Returning `{}` with a 200 turned that mistake into a white page indistinguishable from a dozen other causes,
      // which is how a day went into chasing a blank Services page. It fails loudly instead, and names the screen.
      return NextResponse.json(
        { error: `no page content source for screen '${screen}'` },
        { status: 400 },
      )
    }
  }
}

export const GET = withApiHandler(
  { label: '/api/rust-ui/public-page', route: '/api/rust-ui/public-page' },
  GETHandler,
)
