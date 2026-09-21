import { describe, it, beforeEach, afterEach } from 'node:test'
import assert from 'node:assert/strict'

import { runAdapter } from '../../lib/syndication/adapters'
import { CHANNEL_CATALOG, PREPARE_CHANNELS, isPrepareChannel } from '../../lib/syndication/channels'
import { computeListingSourceHash, isSourceStale } from '../../lib/syndication/hash'
import { diffSnapshot, isOffMarket, launchScore, makeSnapshot, matchesNeedsFilter, placementNeedsMe } from '../../lib/syndication/lifecycle'
import { offMarketShareEn } from '../../lib/syndication/takedown'
import { mediaPublicUrl, photoDownloadBase, photoUrlList } from '../../lib/syndication/media'
import { buildPresenceReport, presenceReportText } from '../../lib/syndication/presence-report'
import { smsBlurb, whatsappBlurb } from '../../lib/syndication/share'
import type { ListingSource, PlacementRow, PlacementStatus, PublishMode, SightingRow, SightingNetwork, SyndicationChannel } from '../../lib/syndication/types'
import {
  buildFacebookHomeListingPayload,
  buildFacebookMarketplaceItemBatch,
} from '../../lib/syndication/facebook'
import { buildResoPropertyPayload, stellarTransportPlan } from '../../lib/syndication/stellar'
import type { ListingSource } from '../../lib/syndication/types'

// ---------------------------------------------------------------------------
// Syndication adapter contracts (spec docs/syndication-adapters-requirements.md
// section 9). Dry-runs must never hit graph.facebook.com — we mock fetch and
// assert call counts. Tokens are supplied via env only at call time.
// ---------------------------------------------------------------------------

const ENV_KEYS = [
  'SYNDICATION_LIVE',
  'META_ACCESS_TOKEN',
  'META_PRODUCT_CATALOG_ID',
  'META_PAGE_ID',
  'META_GRAPH_VERSION',
  'META_MARKETPLACE_PARTNER',
]

function makeSource(overrides: Partial<ListingSource> = {}): ListingSource {
  return {
    id: '11111111-1111-1111-1111-111111111111',
    name: 'Playa Flamenco Villa',
    slug: 'playa-flamenco-villa',
    status: 'active',
    isPublished: true,
    listPrice: 1850000,
    location: 'Flamenco, Culebra',
    city: 'Culebra',
    neighborhood: 'Flamenco',
    bedrooms: 4,
    bathrooms: 4,
    squareFeet: 3200,
    propertyType: 'Villa',
    shortDescription: 'Ocean-view villa above Flamenco.',
    publicRemarks: 'Four-bedroom villa with full ocean view.',
    listingAgentName: 'CulebraLuxe',
    listingAgentPhone: '+1-787-000-0000',
    listingAgentEmail: 'hello@culebraluxe.com',
    publicUrl: 'https://culebraluxe.com/listings/playa-flamenco-villa',
    heroMediaId: null,
    imageCount: 12,
    latitude: 18.33,
    longitude: -65.3,
    yearBuilt: 2018,
    postalCode: '00775',
    streetAddress: '1 Flamenco Rd',
    photos: [
      { mediaId: 'm1', url: 'https://culebraluxe.com/api/media/m1', role: 'hero', sortOrder: 1 },
      { mediaId: 'm2', url: 'https://culebraluxe.com/api/media/m2', role: 'gallery', sortOrder: 2 },
    ],
    ...overrides,
  }
}

function setMetaEnv(live = false) {
  process.env.SYNDICATION_LIVE = live ? 'true' : 'false'
  process.env.META_ACCESS_TOKEN = 'tok-test'
  process.env.META_PRODUCT_CATALOG_ID = 'cat-1'
  process.env.META_PAGE_ID = 'page-1'
  process.env.META_GRAPH_VERSION = 'v21.0'
  process.env.META_MARKETPLACE_PARTNER = ''
}

function stubFetch(calls: Array<{ url: string; headers: Record<string, unknown> }>) {
  ;(globalThis as { fetch: unknown }).fetch = async (input: unknown, init: unknown) => {
    calls.push({
      url: String(input),
      headers: (init as { headers?: Record<string, unknown> })?.headers ?? {},
    })
    return { status: 200, json: async () => ({ id: 'ok' }) }
  }
}

beforeEach(() => {
  for (const key of ENV_KEYS) delete process.env[key]
})

afterEach(() => {
  for (const key of ENV_KEYS) delete process.env[key]
  delete (globalThis as { fetch?: unknown }).fetch
})

describe('syndication adapters', () => {
  it('channel catalog includes stellar_mls as first-class, realtor_com blocked', () => {
    assert.equal(CHANNEL_CATALOG.stellar_mls.mode, 'mls')
    assert.equal(CHANNEL_CATALOG.stellar_mls.readiness, 'pack')
    assert.equal(CHANNEL_CATALOG.pr_mls.readiness, 'blocked')
    assert.equal(CHANNEL_CATALOG.realtor_com.readiness, 'blocked')
    // Facebook dry-runs without env, so it is pack-ready, not a stub.
    assert.equal(CHANNEL_CATALOG.facebook_marketplace.readiness, 'pack')
  })

  it('facebook home_listing payload: Culebra city, PR country, real photos not the listing URL', () => {
    const payload = buildFacebookHomeListingPayload(makeSource()) as {
      home_listing_id: string
      address: Record<string, unknown>
      images: Array<{ image_url: string }>
      year_built: number
    }
    assert.equal(payload.home_listing_id, '11111111-1111-1111-1111-111111111111')
    assert.equal(payload.address.city, 'Culebra')
    assert.equal(payload.address.country, 'PR')
    assert.equal(payload.address.region, 'PR')
    assert.equal(payload.images[0]?.image_url, 'https://culebraluxe.com/api/media/m1')
    assert.equal(payload.images[1]?.image_url, 'https://culebraluxe.com/api/media/m2')
    assert.ok(!payload.images.some((i) => i.image_url.includes('/listings/')))
    assert.equal(payload.year_built, 2018)
  })

  it('facebook omits year_built when unknown (no zero sent)', () => {
    const payload = buildFacebookHomeListingPayload(makeSource({ yearBuilt: null })) as Record<
      string,
      unknown
    >
    assert.equal(payload.year_built, undefined)
  })

  it('facebook items_batch image_link uses the first real photo', () => {
    const batch = buildFacebookMarketplaceItemBatch(makeSource()) as {
      requests: Array<{ data: { image_link: string } }>
    }
    assert.equal(batch.requests[0]?.data.image_link, 'https://culebraluxe.com/api/media/m1')
  })
})


describe('facebook network behavior', () => {
  it('dry-run: no fetch when env is missing (no SYNDICATION_LIVE)', async () => {
    const calls: Array<{ url: string; headers: Record<string, unknown> }> = []
    stubFetch(calls)
    const result = await runAdapter(makeSource(), 'facebook_marketplace')
    assert.equal(result.ok, true)
    assert.equal(result.status, 'pending_manual')
    assert.equal(result.pack.transport?.dryRun, true)
    assert.ok(result.pack.transport?.payload.home_listing)
    assert.equal(calls.length, 0, 'dry-run must not call Meta')
  })

  it('tokens set but SYNDICATION_LIVE=false: still no Graph HTTP, missingEnv empty', async () => {
    setMetaEnv(false) // tokens present, live off
    const calls: Array<{ url: string; headers: Record<string, unknown> }> = []
    stubFetch(calls)
    const result = await runAdapter(makeSource(), 'facebook_marketplace')
    assert.equal(result.pack.transport?.dryRun, true)
    assert.deepEqual(result.pack.transport?.missingEnv, [])
    assert.equal(calls.length, 0)
  })

  it('live guard: SYNDICATION_LIVE=true but no photos => zero fetch calls', async () => {
    setMetaEnv(true)
    const calls: Array<{ url: string; headers: Record<string, unknown> }> = []
    stubFetch(calls)
    const result = await runAdapter(makeSource({ photos: [] }), 'facebook_marketplace')
    assert.equal(result.pack.transport?.dryRun, true)
    assert.equal(
      (result.pack.transport?.response as { reason?: string })?.reason,
      'no_photos',
    )
    assert.equal(calls.length, 0, 'no-photos live must not POST')
  })

  it('live POST attempted: catalog + page feed 2xx => live, two calls, Bearer auth', async () => {
    setMetaEnv(true)
    const calls: Array<{ url: string; headers: Record<string, unknown> }> = []
    stubFetch(calls)
    const result = await runAdapter(makeSource(), 'facebook_marketplace')
    assert.equal(result.status, 'live')
    assert.equal(calls.length, 2, 'expect home_listings + page_feed POSTs')
    assert.ok(!calls[0]?.url.includes('access_token='), 'token must not be in the URL')
    assert.equal(calls[0]?.headers.Authorization, 'Bearer tok-test')
    assert.match(calls[0]?.url ?? '', /\/home_listings$/)
    assert.match(calls[1]?.url ?? '', /\/feed$/)
    // Page /feed body id is surfaced for persistence onto external_id.
    assert.equal(result.externalId, 'ok')
  })

  it('feed-only live: Page /feed works without META_PRODUCT_CATALOG_ID (catalog optional)', async () => {
    setMetaEnv(true)
    delete process.env.META_PRODUCT_CATALOG_ID
    const calls: Array<{ url: string; headers: Record<string, unknown> }> = []
    stubFetch(calls)
    const result = await runAdapter(makeSource(), 'facebook_marketplace')
    assert.equal(result.status, 'live')
    assert.equal(calls.length, 1, 'only Page feed POSTed when catalog id is absent')
    assert.match(calls[0]?.url ?? '', /\/feed$/)
    assert.ok(!(calls[0]?.url ?? '').includes('home_listings'), 'catalog must not be POSTed')
    assert.equal(result.externalId, 'ok')
  })

  it('live POST failure (non-2xx) records failed placement status', async () => {
    setMetaEnv(true)
    ;(globalThis as { fetch: unknown }).fetch = async () => {
      return { status: 400, json: async () => ({ error: { message: 'bad' } }) }
    }
    const result = await runAdapter(makeSource(), 'facebook_marketplace')
    assert.equal(result.status, 'failed')
    assert.equal(result.pack.transport?.dryRun, false)
  })
})

describe('stellar + clasificados', () => {
  it('stellar: never a live write, MFR, realtor distribution on, rich source fields', async () => {
    const source = makeSource()
    const plan = stellarTransportPlan(source)
    const reso = buildResoPropertyPayload(source) as Record<string, unknown>
    assert.equal(plan.liveEnabled, false)
    assert.equal(plan.dryRun, true)
    assert.equal(reso.OriginatingSystemName, 'MFR')
    assert.equal(reso.PostalCode, '00775')
    assert.equal(reso.Latitude, 18.33)
    assert.equal(reso.Longitude, -65.3)
    assert.equal(reso.YearBuilt, 2018)
    assert.equal(reso.PhotosCount, 2)
    const distribution = (plan.payload as { distribution: Record<string, unknown> }).distribution
    assert.equal(distribution.realtorCom, true)
    assert.equal(distribution.zillowRentals, false)
    const result = await runAdapter(source, 'stellar_mls')
    assert.equal(result.status, 'pending_manual')
  })

  it('clasificados: pack only, Spanish body, ttl 40 days', async () => {
    const result = await runAdapter(makeSource(), 'clasificados')
    assert.equal(result.status, 'pending_manual')
    assert.equal(result.mode, 'copy_pack')
    assert.equal(result.ttlDays, 40)
    assert.ok(result.pack.titleEs)
    assert.ok(result.pack.bodyEs)
    assert.match(result.pack.bodyEs, /Detalle/)
    assert.match(result.pack.bodyEs, /Flamenco/)
    assert.equal(result.pack.transport, null)
  })
})

describe('honesty + root fingerprint (V3 §1 / §2.1)', () => {
  it('Prepare channels exclude zillow_fsbo and realtor_com', () => {
    for (const id of ['culebraluxe', 'stellar_mls', 'facebook_marketplace', 'clasificados']) {
      assert.equal(isPrepareChannel(id), true, `${id} should be Prepare-able`)
    }
    assert.ok(!PREPARE_CHANNELS.includes('zillow_fsbo'), 'Zillow must not be a Prepare target')
    assert.ok(!PREPARE_CHANNELS.includes('realtor_com'), 'Realtor.com must not be a Prepare target')
    assert.equal(isPrepareChannel('zillow_fsbo'), false)
    assert.equal(isPrepareChannel('realtor_com'), false)
    assert.equal(isPrepareChannel('hubspot'), false)
  })

  it('source hash changes with price/facts and flags a stale pack', () => {
    const base = makeSource()
    const hash = computeListingSourceHash(base)
    assert.match(hash, /^sh_[0-9a-f]+$/)
    assert.equal(isSourceStale(base, hash), false)
    // price change invalidates the saved pack
    assert.equal(isSourceStale(makeSource({ listPrice: 1900000 }), hash), true)
    // bedroom change invalidates
    assert.equal(isSourceStale(makeSource({ bedrooms: 5 }), hash), true)
    // publish toggle invalidates
    assert.equal(isSourceStale(makeSource({ isPublished: false }), hash), true)
    // null stored hash is never stale
    assert.equal(isSourceStale(makeSource({ listPrice: 1900000 }), null), false)
  })
})

describe('listing lifecycle (V3 §2.2 / §2.7)', () => {
  function row(status: PlacementStatus, over: Partial<PlacementRow> = {}): PlacementRow {
    return {
      id: over.id ?? 'pl-1',
      propertyId: over.propertyId ?? '11111111-1111-1111-1111-111111111111',
      propertyName: 'Playa Flamenco Villa',
      channel: (over.channel ?? 'facebook_marketplace') as SyndicationChannel,
      status,
      publishMode: (over.publishMode ?? 'copy_pack') as PublishMode,
      externalUrl: over.externalUrl ?? null,
      externalId: over.externalId ?? null,
      pack: over.pack ?? {},
      lastError: over.lastError ?? null,
      publishedAt: null,
      expiresAt: null,
      confirmedAt: null,
      lastAttemptAt: null,
      updatedAt: null,
      sourceHash: over.sourceHash ?? null,
    }
  }

  it('isOffMarket flips on unpublished or sold/withdrawn', () => {
    assert.equal(isOffMarket(makeSource()), false)
    assert.equal(isOffMarket(makeSource({ isPublished: false })), true)
    assert.equal(isOffMarket(makeSource({ status: 'sold' })), true)
    assert.equal(isOffMarket(makeSource({ status: 'withdrawn' })), true)
  })

  it('placementNeedsMe: pending/expired/stale need attention; fresh live does not', () => {
    const src = makeSource()
    const current = computeListingSourceHash(src)
    assert.equal(placementNeedsMe(src, row('pending_manual')), true)
    assert.equal(placementNeedsMe(src, row('expired')), true)
    assert.equal(placementNeedsMe(src, row('live', { sourceHash: 'sh_deadbeef' })), true, 'stale live needs attention')
    assert.equal(placementNeedsMe(src, row('live', { sourceHash: current })), false, 'fresh live is fine')
    assert.equal(placementNeedsMe(src, row('live')), false, 'live with no stored hash is fine')
  })

  it('matchesNeedsFilter reflects all/live/expired/needs_me', () => {
    const src = makeSource()
    const fresh = row('live')
    const expired = row('expired')
    assert.equal(matchesNeedsFilter('all', src, fresh), true)
    assert.equal(matchesNeedsFilter('live', src, fresh), true)
    assert.equal(matchesNeedsFilter('live', src, expired), false)
    assert.equal(matchesNeedsFilter('expired', src, expired), true)
    assert.equal(matchesNeedsFilter('needs_me', src, expired), true)
    assert.equal(matchesNeedsFilter('needs_me', src, row('pending_manual')), true)
    assert.equal(matchesNeedsFilter('needs_me', src, row('live', { sourceHash: 'sh_old' })), true)
  })
})

describe('presence report + share blurbs (C / D)', () => {
  const PID = '11111111-1111-1111-1111-111111111111'
  function mkPlacement(channel: SyndicationChannel, status: PlacementStatus, over: Partial<PlacementRow> = {}): PlacementRow {
    return {
      id: over.id ?? 'pl-x',
      propertyId: PID,
      propertyName: 'Playa Flamenco Villa',
      channel,
      status,
      publishMode: (over.publishMode ?? 'copy_pack') as PublishMode,
      externalUrl: over.externalUrl ?? null,
      externalId: over.externalId ?? null,
      pack: over.pack ?? {},
      lastError: over.lastError ?? null,
      publishedAt: null,
      expiresAt: null,
      confirmedAt: null,
      lastAttemptAt: null,
      updatedAt: null,
      sourceHash: over.sourceHash ?? null,
      publishedAtIso: over.publishedAtIso ?? null,
    }
  }

  it('presence report reflects site + Stellar MLS + unobserved Zillow', () => {
    const source = makeSource()
    const stellar = mkPlacement('stellar_mls', 'live', { externalId: 'PR-1234', externalUrl: 'https://stellarmls.example/PR-1234' })
    const report = buildPresenceReport(source, [stellar], [])
    assert.equal(report.propertyName, 'Playa Flamenco Villa')
    assert.equal(report.sitePublished, true)
    assert.equal(report.stellarMls, 'PR-1234')
    assert.match(report.stellarStatus, /In Stellar/)
    assert.equal(report.sightings.length, 0)
    const text = presenceReportText(report)
    assert.match(text, /PR-1234/)
    assert.ok(!text.includes('Published to Zillow'), 'never claims a Zillow publish')
    assert.ok(report.generatedAt.length > 0)
  })

  it('presence report computes days-on-market from an ISO published_at', () => {
    const source = makeSource()
    const iso = new Date(Date.now() - 10 * 86_400_000).toISOString()
    const stellar = mkPlacement('stellar_mls', 'live', { publishedAtIso: iso })
    const report = buildPresenceReport(source, [stellar], [])
    assert.equal(report.daysOnMarket, 10)
    assert.match(presenceReportText(report), /Days on market: 10/)
  })

  it('presence report lists a pasted Zillow sighting as observed', () => {
    const source = makeSource()
    const sighting: SightingRow = {
      id: 's1',
      propertyId: PID,
      network: 'zillow' as SightingNetwork,
      url: 'https://www.zillow.com/homes/123',
      notedAt: null,
      notes: null,
    }
    const report = buildPresenceReport(source, [], [sighting])
    assert.equal(report.sightings.length, 1)
    assert.equal(report.sightings[0]?.network, 'zillow')
    assert.match(presenceReportText(report), /www.zillow.com/)
  })

  it('whatsapp and sms blurbs carry name, price, city and the public URL', () => {
    const source = makeSource()
    const url = 'https://culebraluxe.com/listings/playa-flamenco-villa'
    for (const text of [whatsappBlurb(source, 'en'), smsBlurb(source, 'en')]) {
      assert.match(text, /Playa Flamenco Villa/)
      assert.match(text, /Culebra/)
      assert.ok(text.includes(url), 'blurb includes the public URL')
    }
    assert.match(whatsappBlurb(source, 'es'), /baños/)
    assert.match(whatsappBlurb(source, 'es'), /Detalle/)
  })

describe('next5b polish (tasks 2–5)', () => {
  const PID = '11111111-1111-1111-1111-111111111111'
  function place(channel: SyndicationChannel, status: PlacementStatus, over: Partial<PlacementRow> = {}): PlacementRow {
    return {
      id: over.id ?? 'p',
      propertyId: PID,
      propertyName: 'Playa Flamenco Villa',
      channel,
      status,
      publishMode: 'copy_pack' as PublishMode,
      externalUrl: over.externalUrl ?? null,
      externalId: over.externalId ?? null,
      pack: over.pack ?? {},
      lastError: null,
      publishedAt: null,
      expiresAt: null,
      confirmedAt: null,
      lastAttemptAt: null,
      updatedAt: null,
      sourceHash: over.sourceHash ?? null,
    }
  }

  it('diffSnapshot lists a price change', () => {
    const old = makeSnapshot(makeSource())
    const diffs = diffSnapshot(makeSource({ listPrice: 2350000 }), old)
    assert.ok(diffs.some((d) => d.startsWith('Price:')), diffs.join('|'))
  })

  it('launchScore weighs published/photos/price/city/stellar/facebook', () => {
    const base = makeSource({ heroMediaId: 'h1' })
    const rows = [
      place('stellar_mls', 'live', { externalId: 'PR-1', externalUrl: 'x' }),
      place('facebook_marketplace', 'live', { externalUrl: 'fb-url' }),
    ]
    const full = launchScore(base, rows)
    assert.equal(full.score, 100)
    assert.equal(full.missing.length, 0)
    const partial = launchScore(base, rows.filter((r) => r.channel !== 'facebook_marketplace'))
    assert.equal(partial.score, 90)
  })

  it('Spanish seller report is Spanish; never "Published to Zillow"', () => {
    const report = buildPresenceReport(makeSource(), [], [])
    const es = presenceReportText(report, 'es')
    assert.match(es, /No observada aún/)
    assert.ok(!es.includes('Published to Zillow'))
    assert.ok(presenceReportText(report, 'en').includes('Not observed yet'))
  })

  it('off-market share points at the inventory root when unpublished', () => {
    const text = offMarketShareEn(makeSource({ isPublished: false }))
    assert.ok(text.includes('https://culebraluxe.com'), text)
  })
})


describe('media download helpers (Block A story 1)', () => {
  it('photoDownloadBase slugifies a name/slug and photoUrlList caps at 25', () => {
    assert.equal(photoDownloadBase(null, 'Casa Luar'), 'casa-luar-photos')
    assert.equal(photoDownloadBase('playa-flamenco-villa', 'Anything'), 'playa-flamenco-villa-photos')
    const rows = Array.from({ length: 30 }, (_, i) => ({ media_id: `m${i}` }))
    const list = photoUrlList(rows)
    assert.equal(list.length, 25)
    assert.equal(list[0], '/api/media/m0')
    assert.equal(mediaPublicUrl('m1'), 'https://culebraluxe.com/api/media/m1')
  })
})

})
