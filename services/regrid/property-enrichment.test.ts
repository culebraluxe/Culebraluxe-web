import assert from 'node:assert/strict'
import test from 'node:test'

import { buildRegridPropertyQuery, enrichPropertyFromRegrid } from './property-enrichment'
import type {
  RegridAddressClient,
  RegridParcel,
  RegridPropertyEnrichmentRepository,
  RegridPropertyEnrichmentWrite,
  RegridPropertyTarget,
} from './types'

const TARGET: RegridPropertyTarget = {
  id: 'property-1',
  addressLine1: '8 Calle Example',
  city: 'Culebra',
  stateOrProvince: 'PR',
  postalCode: '00775',
  country: 'Puerto Rico',
  isoCountryCode: 'PR',
  catastroNumber: null,
}

const PARCEL: RegridParcel = {
  llUuid: '11111111-1111-4111-8111-111111111111',
  parcelNumber: '123-456-789-00',
  path: '/us/pr/culebra/example/123',
  headline: '8 Calle Example',
  situsAddress: '8 CALLE EXAMPLE',
  owner: 'EXAMPLE OWNER',
  urbanization: null,
  neighborhood: null,
  zoning: null,
  zoningDescription: null,
  legalDescription: null,
  latitude: 18.3,
  longitude: -65.3,
  lotAcres: 0.4,
  lotSquareFeet: null,
  buildingSquareFeet: 1600,
  yearBuilt: 2002,
  stories: 2,
  bedrooms: 3,
  bathrooms: 2,
  parcelValue: null,
  landValue: null,
  improvementValue: null,
  fields: { parcelnumb: '123-456-789-00' },
  geometry: null,
  rawFeature: { properties: { fields: { parcelnumb: '123-456-789-00' } } },
}

function repository(writes: RegridPropertyEnrichmentWrite[]): RegridPropertyEnrichmentRepository {
  return {
    async getTarget() {
      return TARGET
    },
    async persist(input) {
      writes.push(input)
      return {
        propertyId: input.propertyId,
        catastroNumber: input.parcel.parcelNumber,
        catastroSource: 'regrid',
        regridParcelNumber: input.parcel.parcelNumber,
        regridLlUuid: input.parcel.llUuid,
        regridEnrichedAt: new Date().toISOString(),
      }
    },
  }
}

test('buildRegridPropertyQuery uses canonical Property address facts', () => {
  assert.equal(
    buildRegridPropertyQuery(TARGET),
    '8 Calle Example, Culebra, PR, 00775',
  )
})

test('unique lookup persists exactly once', async () => {
  const writes: RegridPropertyEnrichmentWrite[] = []
  let calls = 0
  const client: RegridAddressClient = {
    async lookupAddress(input) {
      calls += 1
      assert.equal(input.limit, 2)
      return {
        status: 'matched',
        query: input.query,
        path: '/us/pr',
        parcel: PARCEL,
        matches: [PARCEL],
      }
    },
  }

  const result = await enrichPropertyFromRegrid({
    propertyId: TARGET.id,
    client,
    repository: repository(writes),
  })
  assert.equal(calls, 1)
  assert.equal(result.status, 'enriched')
  assert.equal(writes.length, 1)
  assert.equal(writes[0].parcel.parcelNumber, '123-456-789-00')
})

test('ambiguous lookup never mutates Property', async () => {
  const writes: RegridPropertyEnrichmentWrite[] = []
  const client: RegridAddressClient = {
    async lookupAddress(input) {
      return {
        status: 'ambiguous',
        query: input.query,
        path: '/us/pr',
        matches: [PARCEL, { ...PARCEL, parcelNumber: 'OTHER' }],
      }
    },
  }
  const result = await enrichPropertyFromRegrid({
    propertyId: TARGET.id,
    client,
    repository: repository(writes),
  })
  assert.equal(result.status, 'ambiguous')
  assert.equal(writes.length, 0)
})

test('not-found lookup never mutates Property', async () => {
  const writes: RegridPropertyEnrichmentWrite[] = []
  const client: RegridAddressClient = {
    async lookupAddress(input) {
      return { status: 'not_found', query: input.query, path: '/us/pr', matches: [] }
    },
  }
  const result = await enrichPropertyFromRegrid({
    propertyId: TARGET.id,
    client,
    repository: repository(writes),
  })
  assert.equal(result.status, 'not_found')
  assert.equal(writes.length, 0)
})
