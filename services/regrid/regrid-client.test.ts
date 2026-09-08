import assert from 'node:assert/strict'
import test from 'node:test'

import { RegridClient, normalizeRegridParcel } from './regrid-client'

const FEATURE = {
  type: 'Feature',
  geometry: { type: 'Polygon', coordinates: [] },
  properties: {
    headline: '8 Calle Example',
    path: '/us/pr/culebra/example/123',
    ll_uuid: '11111111-1111-4111-8111-111111111111',
    fields: {
      parcelnumb: '123-456-789-00',
      address: '8 CALLE EXAMPLE',
      owner: 'EXAMPLE OWNER',
      urbanization: 'CULEBRA',
      neighborhood: 'PLAYA SARDINAS II',
      lat: '18.3001',
      lon: '-65.3021',
      ll_gisacre: 0.42,
      recrdareano: 1600,
      yearbuilt: 2002,
      numstories: 2,
      num_bedrooms: 3,
      num_bath: 2.5,
      parval: 400000,
    },
  },
}

test('normalizeRegridParcel extracts Catastro candidate and useful property facts', () => {
  const parcel = normalizeRegridParcel(FEATURE)
  assert.equal(parcel.parcelNumber, '123-456-789-00')
  assert.equal(parcel.llUuid, '11111111-1111-4111-8111-111111111111')
  assert.equal(parcel.owner, 'EXAMPLE OWNER')
  assert.equal(parcel.latitude, 18.3001)
  assert.equal(parcel.longitude, -65.3021)
  assert.equal(parcel.lotAcres, 0.42)
  assert.equal(parcel.buildingSquareFeet, 1600)
  assert.equal(parcel.bathrooms, 2.5)
})

test('lookupAddress makes one bounded PR request and keeps token out of URL', async () => {
  let calls = 0
  let calledUrl = ''
  let calledToken = ''
  const client = new RegridClient({
    token: 'SECRET-TOKEN',
    fetchImpl: (async (input, init) => {
      calls += 1
      calledUrl = String(input)
      calledToken = new Headers(init?.headers).get('x-regrid-token') ?? ''
      return new Response(
        JSON.stringify({ parcels: { type: 'FeatureCollection', features: [FEATURE] } }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }) as typeof fetch,
  })

  const result = await client.lookupAddress({ query: '8 Calle Example, Culebra, PR 00775' })
  assert.equal(calls, 1)
  assert.equal(calledToken, 'SECRET-TOKEN')
  assert.doesNotMatch(calledUrl, /SECRET-TOKEN/)
  const url = new URL(calledUrl)
  assert.equal(url.pathname, '/api/v2/parcels/address')
  assert.equal(url.searchParams.get('path'), '/us/pr')
  assert.equal(url.searchParams.get('limit'), '2')
  assert.equal(url.searchParams.get('return_custom'), 'true')
  assert.equal(result.status, 'matched')
})

test('lookupAddress returns not_found without a second request', async () => {
  let calls = 0
  const client = new RegridClient({
    token: 'x',
    fetchImpl: (async () => {
      calls += 1
      return new Response(JSON.stringify({ parcels: { features: [] } }), { status: 200 })
    }) as typeof fetch,
  })
  const result = await client.lookupAddress({ query: 'missing' })
  assert.equal(calls, 1)
  assert.equal(result.status, 'not_found')
})

test('lookupAddress treats two returned parcels as ambiguous in the same request', async () => {
  let calls = 0
  const client = new RegridClient({
    token: 'x',
    fetchImpl: (async () => {
      calls += 1
      return new Response(
        JSON.stringify({
          parcels: {
            features: [
              FEATURE,
              {
                ...FEATURE,
                properties: {
                  ...FEATURE.properties,
                  ll_uuid: '22222222-2222-4222-8222-222222222222',
                  fields: { ...FEATURE.properties.fields, parcelnumb: 'OTHER' },
                },
              },
            ],
          },
        }),
        { status: 200 },
      )
    }) as typeof fetch,
  })
  const result = await client.lookupAddress({ query: 'ambiguous' })
  assert.equal(calls, 1)
  assert.equal(result.status, 'ambiguous')
  if (result.status === 'ambiguous') assert.equal(result.matches.length, 2)
})

test('HTTP failures never place the API token in the thrown error', async () => {
  const client = new RegridClient({
    token: 'DO-NOT-LEAK',
    fetchImpl: (async () => new Response('unauthorized', { status: 401 })) as typeof fetch,
  })
  await assert.rejects(
    () => client.lookupAddress({ query: '8 Calle Example' }),
    (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error)
      assert.match(message, /HTTP 401/)
      assert.doesNotMatch(message, /DO-NOT-LEAK/)
      return true
    },
  )
})
