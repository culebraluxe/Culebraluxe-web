import { missingEnv, readEnv, syndicationLiveEnabled } from './env'
import type { ListingSource, TransportAttempt } from './types'

const GRAPH_VERSION = () => readEnv('META_GRAPH_VERSION') ?? 'v21.0'
const CULEBRA_LAT = 18.303
const CULEBRA_LNG = -65.304

function resolveLat(source: ListingSource): number {
  return typeof source.latitude === 'number' ? source.latitude : CULEBRA_LAT
}

function resolveLng(source: ListingSource): number {
  return typeof source.longitude === 'number' ? source.longitude : CULEBRA_LNG
}

function mapHomePropertyType(value: string | null): string {
  const raw = (value ?? '').toLowerCase()
  if (raw.includes('condo') || raw.includes('apart')) return 'apartment'
  if (raw.includes('land') || raw.includes('lot') || raw.includes('solar')) return 'land'
  if (raw.includes('town')) return 'townhouse'
  if (raw.includes('multi')) return 'apartment'
  return 'house'
}

/** Real photos from the manifest, never the listing HTML URL. */
export function homeListingImages(source: ListingSource): Array<{ image_url: string }> {
  return source.photos.map((photo) => ({ image_url: photo.url }))
}

export function buildFacebookHomeListingPayload(source: ListingSource) {
  const payload: Record<string, unknown> = {
    home_listing_id: source.id,
    name: source.name,
    availability: 'for_sale',
    currency: 'USD',
    price: source.listPrice ?? 0,
    url: source.publicUrl ?? 'https://culebraluxe.com',
    description: source.publicRemarks ?? source.shortDescription ?? source.name,
    num_beds: source.bedrooms,
    num_baths: source.bathrooms,
    property_type: mapHomePropertyType(source.propertyType),
    listing_type: 'for_sale_by_agent',
    address: {
      street_address: source.streetAddress ?? source.location ?? source.name,
      city: source.city ?? 'Culebra',
      region: 'PR',
      country: 'PR',
      neighborhoods: source.neighborhood ? [source.neighborhood] : ['Culebra'],
      latitude: resolveLat(source),
      longitude: resolveLng(source),
    },
    images: homeListingImages(source),
  }
  // Do not send year_built: 0 when unknown.
  if (source.yearBuilt && source.yearBuilt > 0) payload.year_built = source.yearBuilt
  return payload
}

export function buildFacebookMarketplaceItemBatch(source: ListingSource) {
  const price = source.listPrice != null ? `${Math.round(source.listPrice)} USD` : '0 USD'
  const firstImage = source.photos[0]?.url
  return {
    item_type: 'PRODUCT_ITEM',
    requests: [
      {
        method: 'CREATE',
        data: {
          id: source.id,
          title: source.name,
          description: source.publicRemarks ?? source.shortDescription ?? source.name,
          price,
          availability: 'in stock',
          condition: 'new',
          link: source.publicUrl ?? 'https://culebraluxe.com',
          image_link: firstImage ?? source.publicUrl ?? 'https://culebraluxe.com',
          brand: 'CulebraLuxe',
          partner_listing_type: 'real_estate',
          property_type: mapHomePropertyType(source.propertyType),
          sale_type: 'for_sale',
          bed_bath:
            source.bedrooms != null || source.bathrooms != null
              ? `${source.bedrooms ?? 0}/${source.bathrooms ?? 0}`
              : undefined,
          area_size: source.squareFeet != null ? String(source.squareFeet) : undefined,
          listed_by: 'agent',
        },
      },
    ],
  }
}

export function buildFacebookPageFeedPayload(source: ListingSource) {
  const price =
    source.listPrice != null
      ? new Intl.NumberFormat('en-US', {
          style: 'currency',
          currency: 'USD',
          maximumFractionDigits: 0,
        }).format(source.listPrice)
      : 'Price on request'
  return {
    message: [
      `${source.name} — ${price}`,
      source.publicRemarks ?? source.shortDescription ?? '',
      [
        source.bedrooms != null ? `${source.bedrooms} bed` : null,
        source.bathrooms != null ? `${source.bathrooms} bath` : null,
        source.city,
      ]
        .filter(Boolean)
        .join(' · '),
      source.listingAgentName ? `CulebraLuxe · ${source.listingAgentName}` : 'CulebraLuxe',
    ]
      .filter(Boolean)
      .join('\n\n'),
    link: source.publicUrl ?? 'https://culebraluxe.com',
    published: true,
  }
}

/**
 * Plan for the Meta write. Endpoint stays free of the token — auth travels in
 * the Authorization header only, so nothing secret is ever persisted.
 */
export function facebookTransportPlan(source: ListingSource): TransportAttempt {
  const catalogId = readEnv('META_PRODUCT_CATALOG_ID')
  const pageId = readEnv('META_PAGE_ID')
  const token = readEnv('META_ACCESS_TOKEN')
  const live = syndicationLiveEnabled()
  const missing = missingEnv(['META_ACCESS_TOKEN', 'META_PRODUCT_CATALOG_ID', 'META_PAGE_ID'])

  const catalogEndpoint = catalogId
    ? `https://graph.facebook.com/${GRAPH_VERSION()}/${catalogId}/home_listings`
    : `https://graph.facebook.com/${GRAPH_VERSION()}/{META_PRODUCT_CATALOG_ID}/home_listings`
  const feedEndpoint = pageId
    ? `https://graph.facebook.com/${GRAPH_VERSION()}/${pageId}/feed`
    : `https://graph.facebook.com/${GRAPH_VERSION()}/{META_PAGE_ID}/feed`

  const liveAttempt = live && !!(token && catalogId) && source.photos.length > 0

  return {
    kind: 'meta.home_listings+page_feed',
    dryRun: !liveAttempt,
    liveEnabled: live,
    method: 'POST',
    endpoint: catalogEndpoint,
    payload: {
      home_listing: buildFacebookHomeListingPayload(source),
      page_feed: { endpoint: feedEndpoint, body: buildFacebookPageFeedPayload(source) },
      marketplace_items_batch: buildFacebookMarketplaceItemBatch(source),
      notes: [
        'home_listings writes a Real Estate catalog item used by Advantage+ catalog ads and Commerce.',
        'Page /feed is the reliable Graph write you can turn on today with a Page token from the Tech Provider app.',
        'Marketplace items_batch is NOT posted unless META_MARKETPLACE_PARTNER=true (partner listing type real_estate grant).',
      ],
    },
    missingEnv: missing,
  }
}


type PostOutcome = { httpStatus?: number; error?: string; body?: Record<string, unknown> }

async function graphPost(endpoint: string, body: Record<string, unknown>, token: string): Promise<PostOutcome> {
  try {
    const res = await fetch(endpoint, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(body),
    })
    const json = (await res.json().catch(() => ({}))) as Record<string, unknown>
    return { httpStatus: res.status, body: json }
  } catch (error) {
    return { error: error instanceof Error ? error.message : 'Graph request failed' }
  }
}

function is2xx(outcome: PostOutcome | undefined | null): boolean {
  return outcome?.httpStatus != null && outcome.httpStatus >= 200 && outcome.httpStatus < 300
}

export type FacebookLiveResult =
  | { status: 'dry_run'; reason: string }
  | { status: 'live'; homeListings: PostOutcome; pageFeed: PostOutcome }
  | { status: 'partial'; homeListings: PostOutcome; pageFeed: PostOutcome | null }
  | { status: 'failed'; homeListings: PostOutcome; pageFeed: PostOutcome | null }

/**
 * Live order: (1) catalog home_listings, (2) Page /feed. Never items_batch.
 * With no photos we refuse to POST (zero network calls) even when live.
 */
export async function maybePostFacebook(transport: TransportAttempt): Promise<TransportAttempt> {
  const payload = transport.payload as Record<string, unknown>
  const homeListing = (payload.home_listing ?? {}) as Record<string, unknown>
  const feed = (payload.page_feed ?? {}) as { endpoint?: string; body?: Record<string, unknown> }
  const feedEndpoint = feed.endpoint ?? null
  const images = Array.isArray(homeListing.images) ? (homeListing.images as unknown[]) : []

  if (transport.dryRun) {
    const result: FacebookLiveResult = {
      status: 'dry_run',
      reason:
        images.length === 0
          ? 'no_photos'
          : transport.missingEnv.length
            ? 'missing_env'
            : 'SYNDICATION_LIVE is not true',
    }
    return { ...transport, response: result as unknown as Record<string, unknown> }
  }

  const token = readEnv('META_ACCESS_TOKEN')
  if (!token) {
    return {
      ...transport,
      dryRun: true,
      response: { status: 'dry_run', reason: 'missing_token' } as unknown as Record<string, unknown>,
    }
  }
  // Live guard: never POST an imageless listing.
  if (images.length === 0) {
    return {
      ...transport,
      dryRun: true,
      response: { status: 'dry_run', reason: 'no_photos_live_guard' } as unknown as Record<string, unknown>,
    }
  }

  const homeListings = await graphPost(transport.endpoint, homeListing, token)
  const pageFeed =
    feedEndpoint && !feedEndpoint.includes('{') && feed.body
      ? await graphPost(feedEndpoint, feed.body, token)
      : null

  let result: FacebookLiveResult
  if (is2xx(pageFeed)) {
    result = { status: 'live', homeListings, pageFeed: pageFeed as PostOutcome }
  } else if (is2xx(homeListings)) {
    result = { status: 'partial', homeListings, pageFeed }
  } else {
    result = { status: 'failed', homeListings, pageFeed }
  }

  return { ...transport, dryRun: false, response: result as unknown as Record<string, unknown> }
}
