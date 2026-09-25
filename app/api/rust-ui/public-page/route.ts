import { NextResponse, type NextRequest } from 'next/server'

// NEVER CACHED. This route answers with the live public record of a Property — its page, its listing, its similar
// listings. A cached answer here is a Property that was edited in OPS and still reads as it did yesterday, which is
// the class of bug that made a published listing look invisible. `force-dynamic` is the difference between "the site
// reflects the record" and "the site reflects the last build".
export const dynamic = 'force-dynamic'


import { getGuideItems } from '@/legacy/db/guide'
import { getMarketingContent } from '@/legacy/db/marketing-content'
import { getPropertyBySlug, getPublicPropertySlugs, getSimilarProperties } from '@/lib/property-reads'
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
import { withApiHandler, withServerErrorCapture } from '@/lib/error-capture-seam'
import { rustApiPublicListingCopy } from '@/lib/rust-api/client'

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
/**
 * The published listings' taglines, by slug, from the Rust public read. THE CARDS LIVE WITHOUT THEM: a failure is
 * captured durably by the seam (so it is seen) and the page is served with no taglines rather than no page.
 */
const readListingCopy = withServerErrorCapture(
  '/api/rust-ui/public-page:listing-copy',
  rustApiPublicListingCopy,
  { level: 'warn', route: '/api/rust-ui/public-page' },
)

async function taglinesBySlug(): Promise<Map<string, string>> {
  const copy = await readListingCopy().catch(() => [])
  return new Map(copy.map((entry) => [entry.slug, entry.tagline]))
}

function listing(source: PropertySummary, taglines: Map<string, string> = new Map()) {
  return {
    id: source.id ?? '',
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
    // `area` is the LOT. A house is bought by its interior first, and without this a 6,000 sq ft residence read as "1 Acre"
    // on its card. Formatted by the same helper, so "6,399 SF" matches the detail page.
    interiorArea: source.squareFeet ? formatArea(source.squareFeet, 'SF') : null,
    views: source.views ?? [],
    beachAccess: source.beachAccess === true,
    tagline: taglines.get(source.slug ?? '') ?? null,
    featured: source.featured === true,
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const screen = req.nextUrl.searchParams.get('screen') ?? ''

  switch (screen) {
    case 'site-home': {
      // The same two reads the live homepage made, at the same time, each with its own failure.
      const [propertiesResult, contentResult, taglines] = await Promise.all([
        getProperties({ publicOnly: true }),
        getMarketingContent(),
        taglinesBySlug(),
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
        featured: properties
          .filter((property) => property.featured === true)
          .map((property) => listing(property, taglines)),
        listings: properties.map((property) => listing(property, taglines)),
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
        listings: properties.map((property) => listing(property)),
        featured: properties.filter((property) => property.featured === true).map((property) => listing(property)),
      })
    }
    case 'site-favorites': {
      // SAVED PROPERTIES LIVE IN THE BROWSER, so the page is the published inventory and the browser picks the saved
      // ones out of it. A saved listing that has since been unpublished is simply not in this list, so it cannot show.
      const result = await getProperties({ publicOnly: true })
      return NextResponse.json({ listings: result.ok ? result.data.map((property) => listing(property)) : [] })
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
      // An enquiry about one property names it, as the live form did ("Request a private viewing of ..."). The id comes
      // from the page's URL, so it is looked up among the PUBLISHED listings only: an unpublished property's name is not
      // something a guessed id may reveal.
      const propertyId = req.nextUrl.searchParams.get('scope')
      let enquiryProperty: string | null = null
      if (propertyId) {
        const listings = await getProperties({ publicOnly: true })
        enquiryProperty = listings.ok
          ? (listings.data.find((property) => property.id === propertyId)?.name ?? null)
          : null
      }
      return NextResponse.json({
        hero: block(contact.hero),
        contact: block(contact.contact),
        enquiryProperty,
      })
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
    case 'site-property-detail': {
      // THE PROPERTY RECORD, IN FULL. This is the one page that is neither a list nor editorial copy: a cockpit with the
      // media panel, a facts card and the actions, four tabs of description, location, documents and video, and the two
      // rails beside it. The rows route flattened it into a fact table, which is what the child page was showing.
      const slug = req.nextUrl.searchParams.get('scope')
      if (!slug) {
        return NextResponse.json({ error: 'a property record needs a slug' }, { status: 400 })
      }
      const result = await getPropertyBySlug(slug)
      if (!result.ok) {
        return NextResponse.json(
          { error: `the property could not be read: ${result.error.kind}` },
          { status: 502 },
        )
      }
      if (!result.data) {
        // A slug that matches nothing is a 404, not an empty page: the component calls `notFound()` for the same case.
        return NextResponse.json({ error: `no property with the slug '${slug}'` }, { status: 404 })
      }
      const { property, heroUrl, galleryImages, videos, documents } = result.data
      const [similarResult, slugsResult] = await Promise.all([
        getSimilarProperties(property._id, {
          propertyType: property.propertyType ?? null,
          city: property.city ?? null,
          neighborhood: property.neighborhood ?? null,
          listPrice: property.listPrice ?? null,
        }),
        getPublicPropertySlugs(),
      ])
      // THE FIELD NAMES ARE THE READ MODEL'S, NOT GUESSED — which is the lesson this route already carries in its header
      // and which I ignored once: `slug`, `name`, `bedrooms`, `bathrooms` and `lotSize` do not exist on a PropertyDetail.
      // It carries `_id`, `title`, `bedroomsTotal`, `bathroomsTotal`, `lotSizeArea` and `livingArea`, and the compiler
      // caught every one of my inventions.
      const location = [property.neighborhood, property.city, property.stateOrProvince].filter(Boolean).join(', ')
      // PageContent owns ONE `property: PropertyRecord`. Media belongs inside that record.
      // Returning hero/gallery/video/documents beside `property` silently discarded them during Rust
      // deserialization, which is why Casa Luar rendered its facts but the hero fell back to the gray placeholder.
      return NextResponse.json({
        property: {
          id: property._id,
          // The slug is the key the page was asked for; the record itself carries only its id.
          slug,
          title: property.title ?? slug,
          kind: property.propertyType ?? null,
          price: property.listPrice == null ? null : formatPrice(property.listPrice),
          beds: property.bedroomsTotal ?? null,
          baths: property.bathroomsTotal ?? null,
          area: property.livingArea == null ? null : `${property.livingArea} sq ft`,
          location: location || null,
          description: property.editorialDescription ?? property.shortDescription ?? null,
          shortDescription: property.shortDescription ?? null,
          yearBuilt: property.yearBuilt ?? null,
          architecture: property.architecture ?? null,
          status: property.standardStatus ?? null,
          neighborhood: property.neighborhood ?? null,
          city: property.city ?? null,
          stateOrProvince: property.stateOrProvince ?? null,
          lotSize: formatArea(property.lotSizeArea, property.lotSizeUnits),
          lotSizeSqft: property.lotSizeSqft ?? null,
          roadFrontageFeet: property.roadFrontageFeet ?? null,
          roadSurfaceType: property.roadSurfaceType ?? null,
          lotDescription: property.lotDescription ?? null,
          utilitiesNotes: property.utilitiesNotes ?? null,
          livingArea: property.livingArea ?? null,
          bathroomsFull: property.bathroomsFull ?? null,
          bathroomsHalf: property.bathroomsHalf ?? null,
          stories: property.stories ?? null,
          parkingSpaces: property.parkingSpaces ?? null,
          waterAccess: property.waterAccess ?? false,
          beachAccess: property.beachAccess ?? false,
          amenities: property.amenities ?? [],
          viewType: property.viewType ?? [],
          lifestyleTags: property.lifestyleTags ?? [],
          listingAgentName: property.listingAgentName ?? null,
          listingAgentPhone: property.listingAgentPhone ?? null,
          listingAgentEmail: property.listingAgentEmail ?? null,
          listingOffice: property.listingOffice ?? null,
          listingId: property.listingId ?? null,
          latitude: property.latitude ?? null,
          longitude: property.longitude ?? null,
          heroUrl: heroUrl ?? null,
          gallery: galleryImages ?? [],
          videos: videos ?? [],
          documents: documents ?? [],
          similar: similarResult.ok ? similarResult.data.map((property) => listing(property)) : [],
          publicSlugs: slugsResult.ok ? slugsResult.data : [],
        },
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
