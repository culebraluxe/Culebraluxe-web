import { sql } from './client'
import { CHANNEL_CATALOG, type SyndicationChannel } from '@/lib/syndication/channels'
import { runAdapter } from '@/lib/syndication/adapters'
import { buildPhotoManifest, type PhotoDbRow } from '@/lib/syndication/photos'
import { computeListingSourceHash } from '@/lib/syndication/hash'
import type {
  ListingPack,
  ListingSource,
  PhotoManifestItem,
  PlacementRow,
  SightingNetwork,
  SightingRow,
  SyndicationEventRow,
} from '@/lib/syndication/types'

const PUBLIC_ORIGIN = 'https://culebraluxe.com'

type SourceRaw = {
  id: string
  name: string
  slug: string | null
  status: string
  is_published: boolean
  list_price: string | null
  location: string | null
  city: string | null
  neighborhood: string | null
  bedrooms: string | null
  bathrooms: string | null
  square_feet: number | null
  property_type: string | null
  short_description: string | null
  public_remarks: string | null
  listing_agent_name: string | null
  listing_agent_phone: string | null
  listing_agent_email: string | null
  hero_media_id: string | null
  image_count: number
  latitude: string | number | null
  longitude: string | number | null
  year_built: string | number | null
  postal_code: string | null
  street_address: string | null
}

function toNumber(value: string | null): number | null {
  if (value === null) return null
  const n = Number(value)
  return Number.isFinite(n) ? n : null
}

function toNumberOrNull(value: string | number | null): number | null {
  if (value === null || value === '') return null
  const n = Number(value)
  return Number.isFinite(n) ? n : null
}

function mapSource(row: SourceRaw, photos: PhotoManifestItem[] = []): ListingSource {
  return {
    id: row.id,
    name: row.name,
    slug: row.slug,
    status: row.status,
    isPublished: row.is_published,
    listPrice: toNumber(row.list_price),
    location: row.location,
    city: row.city,
    neighborhood: row.neighborhood,
    bedrooms: toNumber(row.bedrooms),
    bathrooms: toNumber(row.bathrooms),
    squareFeet: row.square_feet,
    propertyType: row.property_type,
    shortDescription: row.short_description,
    publicRemarks: row.public_remarks,
    listingAgentName: row.listing_agent_name,
    listingAgentPhone: row.listing_agent_phone,
    listingAgentEmail: row.listing_agent_email,
    publicUrl: row.slug ? `${PUBLIC_ORIGIN}/listings/${row.slug}` : null,
    heroMediaId: row.hero_media_id,
    imageCount: Number(row.image_count ?? 0),
    latitude: toNumberOrNull(row.latitude),
    longitude: toNumberOrNull(row.longitude),
    yearBuilt: toNumberOrNull(row.year_built),
    postalCode: row.postal_code,
    streetAddress: row.street_address || null,
    photos,
  }
}

/** Resolve the ordered image manifest for one property (hero first, cap 25). */
async function loadPropertyPhotos(propertyId: string): Promise<PhotoManifestItem[]> {
  const rows = (await sql`
    select m.id as media_id, pm.role, pm.sort_order, m.width, m.height, m.mime_type
    from property_media pm
    join media m on m.id = pm.media_id
    where pm.property_id = ${propertyId} and m.media_type = 'image'
  `) as PhotoDbRow[]
  return buildPhotoManifest(rows)
}

export async function listListingSources(): Promise<ListingSource[]> {
  const rows = (await sql`
    select p.id, p.name, p.slug, p.status, p.is_published, p.list_price,
      p.location, p.city, p.neighborhood, p.bedrooms, p.bathrooms, p.square_feet,
      p.property_type, p.short_description, p.public_remarks,
      p.listing_agent_name, p.listing_agent_phone, p.listing_agent_email,
      p.latitude, p.longitude, p.year_built, p.postal_code,
      trim(concat_ws(' ', p.street_number, p.street_name, p.unit_number)) as street_address,
      (select pm.media_id from property_media pm
        where pm.property_id = p.id and pm.role = 'hero'
        order by pm.sort_order asc, pm.created_at asc limit 1) as hero_media_id,
      (select count(*)::int from property_media pm
        join media m on m.id = pm.media_id
        where pm.property_id = p.id and m.media_type = 'image') as image_count
    from property p
    where p.archived_at is null
    order by p.is_published desc, p.updated_at desc, p.name asc
  `) as SourceRaw[]
  return rows.map((row) => mapSource(row))
}

export async function getListingSource(id: string): Promise<ListingSource | null> {
  const rows = (await sql`
    select p.id, p.name, p.slug, p.status, p.is_published, p.list_price,
      p.location, p.city, p.neighborhood, p.bedrooms, p.bathrooms, p.square_feet,
      p.property_type, p.short_description, p.public_remarks,
      p.listing_agent_name, p.listing_agent_phone, p.listing_agent_email,
      p.latitude, p.longitude, p.year_built, p.postal_code,
      trim(concat_ws(' ', p.street_number, p.street_name, p.unit_number)) as street_address,
      (select pm.media_id from property_media pm
        where pm.property_id = p.id and pm.role = 'hero'
        order by pm.sort_order asc, pm.created_at asc limit 1) as hero_media_id,
      (select count(*)::int from property_media pm
        join media m on m.id = pm.media_id
        where pm.property_id = p.id and m.media_type = 'image') as image_count
    from property p
    where p.archived_at is null and p.id = ${id}
    limit 1
  `) as SourceRaw[]
  if (!rows[0]) return null
  const photos = await loadPropertyPhotos(rows[0].id)
  return mapSource(rows[0], photos)
}

function mapPlacement(row: {
  id: string; property_id: string; property_name: string; channel: SyndicationChannel
  status: PlacementRow['status']; publish_mode: PlacementRow['publishMode']
  external_url: string | null; external_id: string | null
  pack: ListingPack | Record<string, never> | null; last_error: string | null
  source_hash: string | null
  published_at_iso: string | null
  published_at: string | null; expires_at: string | null; confirmed_at: string | null
  last_attempt_at: string | null; updated_at: string | null
}): PlacementRow {
  return {
    id: row.id, propertyId: row.property_id, propertyName: row.property_name,
    channel: row.channel, status: row.status, publishMode: row.publish_mode,
    externalUrl: row.external_url, externalId: row.external_id, pack: row.pack ?? {},
    lastError: row.last_error, sourceHash: row.source_hash,
    publishedAtIso: row.published_at_iso ?? null,
    publishedAt: row.published_at, expiresAt: row.expires_at,
    confirmedAt: row.confirmed_at, lastAttemptAt: row.last_attempt_at, updatedAt: row.updated_at,
  }
}

export async function listPlacements(): Promise<PlacementRow[]> {
  try {
    const rows = (await sql`
      select s.id, s.property_id, p.name as property_name, s.channel, s.status,
        s.publish_mode, s.external_url, s.external_id, s.pack, s.last_error, s.source_hash,
        s.published_at as published_at_iso,
        to_char(s.published_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as published_at,
        to_char(s.expires_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY') as expires_at,
        to_char(s.confirmed_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as confirmed_at,
        to_char(s.last_attempt_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as last_attempt_at,
        to_char(s.updated_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as updated_at
      from listing_syndication_placement s
      join property p on p.id = s.property_id
      order by s.updated_at desc
    `) as Parameters<typeof mapPlacement>[0][]
    return rows.map(mapPlacement)
  } catch {
    return []
  }
}

export type MarketingDashboardSnapshot = {
  publishedOnSite: number; inventory: number; livePlacements: number
  pendingManual: number; expired: number; failed: number
}

export async function getMarketingDashboard(): Promise<MarketingDashboardSnapshot> {
  const [inventory] = (await sql`
    select count(*) filter (where archived_at is null)::int as inventory,
      count(*) filter (where archived_at is null and is_published)::int as published_on_site
    from property
  `) as Array<{ inventory: number; published_on_site: number }>
  let live = 0, pending = 0, expired = 0, failed = 0
  try {
    const [counts] = (await sql`
      select count(*) filter (where status = 'live')::int as live,
        count(*) filter (where status = 'pending_manual')::int as pending_manual,
        count(*) filter (where status = 'expired')::int as expired,
        count(*) filter (where status = 'failed')::int as failed
      from listing_syndication_placement
    `) as Array<{ live: number; pending_manual: number; expired: number; failed: number }>
    live = counts?.live ?? 0
    pending = counts?.pending_manual ?? 0
    expired = counts?.expired ?? 0
    failed = counts?.failed ?? 0
  } catch { /* migration 098 not applied */ }
  return {
    publishedOnSite: inventory?.published_on_site ?? 0,
    inventory: inventory?.inventory ?? 0,
    livePlacements: live, pendingManual: pending, expired, failed,
  }
}

export async function requestPublish(input: {
  propertyId: string; channel: SyndicationChannel; externalUrl?: string | null
}): Promise<{ ok: boolean; error?: string; placementId?: string; message?: string }> {
  const source = await getListingSource(input.propertyId)
  if (!source) return { ok: false, error: 'Property not found.' }
  const result = await runAdapter(source, input.channel)
  const def = CHANNEL_CATALOG[input.channel]
  const sourceHash = computeListingSourceHash(source)
  const expiresAt = result.ttlDays != null
    ? new Date(Date.now() + result.ttlDays * 24 * 60 * 60 * 1000).toISOString() : null
  const publishedAt = result.status === 'live' ? new Date().toISOString() : null
  const eventType = result.status === 'pending_manual' ? 'pack_generated'
    : result.status === 'live' ? 'marked_live' : result.ok ? 'publish_requested' : 'failed'
  try {
    const rows = (await sql`
      insert into listing_syndication_placement (
        property_id, channel, status, publish_mode, pack, last_error,
        last_attempt_at, published_at, expires_at, external_url, external_id,
        source_hash, updated_at
      ) values (
        ${input.propertyId}, ${input.channel}, ${result.status}, ${result.mode},
        ${JSON.stringify(result.pack)}, ${result.ok ? null : result.message}, now(),
        ${publishedAt}, ${expiresAt}, ${input.externalUrl ?? null}, ${result.externalId ?? null},
        ${sourceHash}, now()
      )
      on conflict (property_id, channel) do update set
        status = excluded.status, publish_mode = excluded.publish_mode,
        pack = excluded.pack, last_error = excluded.last_error,
        last_attempt_at = now(),
        published_at = coalesce(excluded.published_at, listing_syndication_placement.published_at),
        expires_at = excluded.expires_at,
        external_url = coalesce(excluded.external_url, listing_syndication_placement.external_url),
        external_id = coalesce(excluded.external_id, listing_syndication_placement.external_id),
        source_hash = excluded.source_hash,
        updated_at = now()
      returning id
    `) as Array<{ id: string }>
    const placementId = rows[0]?.id
    if (!placementId) return { ok: false, error: 'Could not write placement.' }
    await sql`
      insert into listing_syndication_event (placement_id, event_type, detail)
      values (${placementId}, ${eventType}, ${JSON.stringify({
        message: result.message, mode: result.mode, readiness: def.readiness,
        dryRun: result.transport?.dryRun ?? null,
        transportKind: result.transport?.kind ?? null,
      })})
    `
    return {
      ok: result.ok || result.status === 'pending_manual',
      placementId, message: result.message,
      error: result.ok || result.status === 'pending_manual' ? undefined : result.message,
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Syndication ledger write failed.'
    return {
      ok: false,
      error: message.includes('does not exist')
        ? 'Migration 098/099 is not applied yet.'
        : message,
    }
  }
}

export async function requestPublishMany(input: {
  propertyId: string; channels: SyndicationChannel[]
}) {
  if (input.channels.length === 0) {
    return { ok: false, error: 'Select at least one channel.', results: [] as const }
  }
  const results = []
  for (const channel of input.channels) {
    results.push({ channel, ...(await requestPublish({ propertyId: input.propertyId, channel })) })
  }
  const ok = results.every((r) => r.ok)
  return { ok, error: ok ? undefined : results.find((r) => !r.ok)?.error, results }
}

export async function confirmPlacement(input: {
  placementId: string; externalUrl?: string | null
}) {
  const rows = (await sql`
    update listing_syndication_placement
    set status = 'live', confirmed_at = now(), published_at = coalesce(published_at, now()),
      external_url = coalesce(${input.externalUrl ?? null}, external_url),
      last_error = null, updated_at = now()
    where id = ${input.placementId}
    returning id
  `) as Array<{ id: string }>
  if (!rows[0]) return { ok: false, error: 'Placement not found.' }
  await sql`
    insert into listing_syndication_event (placement_id, event_type, detail)
    values (${input.placementId}, 'confirmed', ${JSON.stringify({ externalUrl: input.externalUrl ?? null })})
  `
  return { ok: true }
}

export async function withdrawPlacement(placementId: string) {
  const rows = (await sql`
    update listing_syndication_placement
    set status = 'withdrawn', updated_at = now()
    where id = ${placementId} returning id
  `) as Array<{ id: string }>
  if (!rows[0]) return { ok: false, error: 'Placement not found.' }
  await sql`insert into listing_syndication_event (placement_id, event_type, detail) values (${placementId}, 'withdrawn', '{}')`
  return { ok: true }
}

/** Renew an expired Clasificados/Facebook placement (re-opens the pack window). */
export async function renewPlacement(placementId: string) {
  const found = (await sql`
    select channel from listing_syndication_placement where id = ${placementId}
  `) as Array<{ channel: SyndicationChannel }>
  const channel = found[0]?.channel
  if (!channel) return { ok: false, error: 'Placement not found.' }
  if (channel !== 'clasificados' && channel !== 'facebook_marketplace') {
    return { ok: false, error: 'Only Clasificados/Facebook placements expire and renew.' }
  }
  const def = CHANNEL_CATALOG[channel]
  const ttlDays = def.defaultTtlDays ?? 30
  const expiresAt = new Date(Date.now() + ttlDays * 24 * 60 * 60 * 1000).toISOString()
  const rows = (await sql`
    update listing_syndication_placement
    set status = 'pending_manual', last_error = null, last_attempt_at = now(),
      expires_at = ${expiresAt}, updated_at = now()
    where id = ${placementId} and status = 'expired'
    returning id
  `) as Array<{ id: string }>
  if (!rows[0]) return { ok: false, error: 'Only an expired placement can be renewed.' }
  await sql`
    insert into listing_syndication_event (placement_id, event_type, detail)
    values (${placementId}, 'renewed', ${JSON.stringify({ ttlDays })})
  `
  return { ok: true }
}

export async function listRecentSyndicationEvents(limit = 16): Promise<
  Array<SyndicationEventRow & { channel: SyndicationChannel; propertyName: string }>
> {
  try {
    const rows = (await sql`
      select e.id, e.placement_id, e.event_type, e.detail, s.channel, p.name as property_name,
        to_char(e.created_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as created_at
      from listing_syndication_event e
      join listing_syndication_placement s on s.id = e.placement_id
      join property p on p.id = s.property_id
      order by e.created_at desc
      limit ${limit}
    `) as Array<{
      id: string; placement_id: string; event_type: string; detail: Record<string, unknown>
      channel: SyndicationChannel; property_name: string; created_at: string
    }>
    return rows.map((row) => ({
      id: row.id, placementId: row.placement_id, eventType: row.event_type,
      detail: row.detail ?? {}, createdAt: row.created_at, channel: row.channel,
      propertyName: row.property_name,
    }))
  } catch {
    return []
  }
}

const SIGHTING_NETWORKS: readonly SightingNetwork[] = ['zillow', 'realtor_com', 'homes_com', 'other']

/**
 * Record an observed destination (V3 §2.3). A pasted public URL — this is NOT a
 * Publish action and never creates a placement.
 */
export async function addSighting(input: {
  propertyId: string
  network: SightingNetwork
  url: string
  notes?: string | null
}): Promise<{ ok: boolean; error?: string }> {
  if (!SIGHTING_NETWORKS.includes(input.network)) {
    return { ok: false, error: `Unknown network: ${input.network}` }
  }
  try {
    await sql`
      insert into listing_syndication_sighting (property_id, network, url, notes)
      values (${input.propertyId}, ${input.network}, ${input.url}, ${input.notes ?? null})
    `
    return { ok: true }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Sighting write failed.'
    return {
      ok: false,
      error: message.includes('does not exist') ? 'Migration 101 is not applied yet.' : message,
    }
  }
}

export async function listSightings(propertyId?: string): Promise<SightingRow[]> {
  try {
    const rows = (propertyId
      ? await sql`
          select id, property_id, network, url, notes,
            to_char(noted_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as noted_at
          from listing_syndication_sighting
          where property_id = ${propertyId}
          order by noted_at desc
        `
      : await sql`
          select id, property_id, network, url, notes,
            to_char(noted_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as noted_at
          from listing_syndication_sighting
          order by noted_at desc
        `) as Array<{
      id: string
      property_id: string
      network: SightingNetwork
      url: string
      notes: string | null
      noted_at: string | null
    }>
    return rows.map((row) => ({
      id: row.id,
      propertyId: row.property_id,
      network: row.network,
      url: row.url,
      notedAt: row.noted_at,
      notes: row.notes,
    }))
  } catch {
    return []
  }
}
