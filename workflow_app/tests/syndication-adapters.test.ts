import { describe, expect, it } from 'vitest'
import { runAdapter } from '@/lib/syndication/adapters'
import { CHANNEL_CATALOG } from '@/lib/syndication/channels'
import { buildFacebookHomeListingPayload } from '@/lib/syndication/facebook'
import { buildResoPropertyPayload } from '@/lib/syndication/stellar'
import type { ListingSource } from '@/lib/syndication/types'

const source: ListingSource = {
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
}

describe('syndication adapters', () => {
  it('builds a Meta home listing payload with Culebra coordinates', () => {
    const payload = buildFacebookHomeListingPayload(source)
    expect(payload.home_listing_id).toBe(source.id)
    expect(payload.availability).toBe('for_sale')
    expect(payload.address.city).toBe('Culebra')
    expect(payload.address.country).toBe('PR')
    expect(payload.price).toBe(1850000)
  })

  it('builds a RESO Property payload for Matrix', () => {
    const payload = buildResoPropertyPayload(source)
    expect(payload.ListPrice).toBe(1850000)
    expect(payload.City).toBe('Culebra')
    expect(payload.StateOrProvince).toBe('PR')
    expect(payload.BedroomsTotal).toBe(4)
    expect(payload.OriginatingSystemName).toBe('MFR')
  })

  it('facebook adapter stores a dry-run transport without posting', async () => {
    const result = await runAdapter(source, 'facebook_marketplace')
    expect(result.ok).toBe(true)
    expect(result.status).toBe('pending_manual')
    expect(result.pack.transport?.dryRun).toBe(true)
    expect(result.pack.transport?.payload.home_listing).toBeTruthy()
  })

  it('stellar adapter never claims a live write', async () => {
    const result = await runAdapter(source, 'stellar_mls')
    expect(result.ok).toBe(true)
    expect(result.mode).toBe('mls')
    expect(result.pack.transport?.liveEnabled).toBe(false)
    expect(result.pack.transport?.payload.distribution).toBeTruthy()
  })

  it('catalog includes stellar as a selectable channel', () => {
    expect(CHANNEL_CATALOG.stellar_mls.mode).toBe('mls')
    expect(CHANNEL_CATALOG.realtor_com.readiness).toBe('blocked')
  })
})
