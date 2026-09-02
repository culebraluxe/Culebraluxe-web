import { missingEnv, readEnv, syndicationLiveEnabled } from './env'
import type { ListingSource, TransportAttempt } from './types'

const GRAPH_VERSION = () => readEnv('META_GRAPH_VERSION') ?? 'v21.0'
const CULEBRA_LAT = 18.303
const CULEBRA_LNG = -65.304

function mapHomePropertyType(value: string | null): string {
  const raw = (value ?? '').toLowerCase()
  if (raw.includes('condo') || raw.includes('apart')) return 'apartment'
  if (raw.includes('land') || raw.includes('lot') || raw.includes('solar')) return 'land'
  if (raw.includes('town')) return 'townhouse'
  if (raw.includes('multi')) return 'apartment'
  return 'house'
}

export function buildFacebookHomeListingPayload(source: ListingSource) {
  return {
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
      street_address: source.location ?? source.name,
      city: source.city ?? 'Culebra',
      region: 'PR',
      country: 'PR',
      neighborhoods: source.neighborhood ? [source.neighborhood] : ['Culebra'],
      latitude: CULEBRA_LAT,
      longitude: CULEBRA_LNG,
    },
    images: source.publicUrl ? [{ image_url: source.publicUrl }] : [],
  }
}

export function buildFacebookMarketplaceItemBatch(source: ListingSource) {
  const price = source.listPrice != null ? `${Math.round(source.listPrice)} USD` : '0 USD'
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
          image_link: source.publicUrl ?? 'https://culebraluxe.com',
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
      [source.bedrooms != null ? `${source.bedrooms} bed` : null, source.bathrooms != null ? `${source.bathrooms} bath` : null, source.city]
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

export function facebookTransportPlan(source: ListingSource): TransportAttempt {
  const catalogId = readEnv('META_PRODUCT_CATALOG_ID')
  const pageId = readEnv('META_PAGE_ID')
  const token = readEnv('META_ACCESS_TOKEN')
  const live = syndicationLiveEnabled()
  const missing = missingEnv(['META_ACCESS_TOKEN', 'META_PRODUCT_CATALOG_ID', 'META_PAGE_ID'])
  const catalogEndpoint = catalogId
    ? `https://graph.facebook.com/${GRAPH_VERSION()}/${catalogId}/home_listings`
    : `https://graph.facebook.com/${GRAPH_VERSION()}/{META_PRODUCT_CATALOG_ID}/home_listings`
  return {
    kind: 'meta.home_listings+page_feed',
    dryRun: !(live && token && catalogId),
    liveEnabled: live,
    method: 'POST',
    endpoint: catalogEndpoint,
    payload: {
      home_listing: buildFacebookHomeListingPayload(source),
      marketplace_items_batch: buildFacebookMarketplaceItemBatch(source),
      page_feed: {
        endpoint: pageId
          ? `https://graph.facebook.com/${GRAPH_VERSION()}/${pageId}/feed`
          : `https://graph.facebook.com/${GRAPH_VERSION()}/{META_PAGE_ID}/feed`,
        body: buildFacebookPageFeedPayload(source),
      },
      notes: [
        'home_listings writes a Real Estate catalog item used by Advantage+ catalog ads and Commerce.',
        'Marketplace consumer listings still have no public broker POST. items_batch only works if Meta granted Marketplace partner listing type real_estate.',
        'Page /feed is the reliable Graph write you can turn on today with a Page token from the Tech Partner app.',
      ],
    },
    missingEnv: missing,
  }
}

export async function maybePostFacebook(transport: TransportAttempt): Promise<TransportAttempt> {
  if (transport.dryRun) {
    return {
      ...transport,
      response: { status: 'dry_run', reason: transport.missingEnv.length ? 'missing_env' : 'SYNDICATION_LIVE is not true' },
    }
  }
  const token = readEnv('META_ACCESS_TOKEN')
  if (!token) {
    return { ...transport, dryRun: true, response: { status: 'dry_run', reason: 'missing_token' } }
  }
  const homeListing = transport.payload.home_listing as Record<string, unknown>
  try {
    const res = await fetch(`${transport.endpoint}?access_token=${encodeURIComponent(token)}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(homeListing),
    })
    const json = (await res.json().catch(() => ({}))) as Record<string, unknown>
    return { ...transport, dryRun: false, response: { httpStatus: res.status, body: json } }
  } catch (error) {
    return {
      ...transport,
      dryRun: false,
      response: { status: 'failed', error: error instanceof Error ? error.message : 'Graph request failed' },
    }
  }
}
