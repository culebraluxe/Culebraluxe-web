import { missingEnv, readEnv, syndicationLiveEnabled } from './env'
import type { ListingSource, TransportAttempt } from './types'

export function buildHubSpotListingProperties(source: ListingSource) {
  return {
    listing_name: source.name,
    slug: source.slug,
    list_price: source.listPrice,
    city: source.city ?? 'Culebra',
    neighborhood: source.neighborhood,
    bedrooms: source.bedrooms,
    bathrooms: source.bathrooms,
    square_feet: source.squareFeet,
    property_type: source.propertyType,
    public_url: source.publicUrl,
    status: source.isPublished ? 'published' : source.status,
    listing_agent: source.listingAgentName,
    listing_agent_email: source.listingAgentEmail,
    source_property_id: source.id,
  }
}

export function hubspotTransportPlan(source: ListingSource): TransportAttempt {
  const token = readEnv('HUBSPOT_ACCESS_TOKEN')
  const objectType = readEnv('HUBSPOT_LISTING_OBJECT') ?? 'p_listings'
  const live = syndicationLiveEnabled()
  const missing = missingEnv(['HUBSPOT_ACCESS_TOKEN'])
  return {
    kind: 'hubspot.crm_object',
    dryRun: !(live && token),
    liveEnabled: live,
    method: 'POST',
    endpoint: `https://api.hubapi.com/crm/v3/objects/${objectType}`,
    payload: { properties: buildHubSpotListingProperties(source) },
    missingEnv: missing,
  }
}

export async function maybePostHubSpot(transport: TransportAttempt): Promise<TransportAttempt> {
  if (transport.dryRun) {
    return {
      ...transport,
      response: {
        status: 'dry_run',
        reason: transport.missingEnv.length ? 'missing_env' : 'SYNDICATION_LIVE is not true',
      },
    }
  }
  const token = readEnv('HUBSPOT_ACCESS_TOKEN')
  if (!token) {
    return { ...transport, dryRun: true, response: { status: 'dry_run', reason: 'missing_token' } }
  }
  try {
    const res = await fetch(transport.endpoint, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(transport.payload),
    })
    const json = (await res.json().catch(() => ({}))) as Record<string, unknown>
    return { ...transport, response: { httpStatus: res.status, body: json } }
  } catch (error) {
    return {
      ...transport,
      response: {
        status: 'failed',
        error: error instanceof Error ? error.message : 'HubSpot request failed',
      },
    }
  }
}
