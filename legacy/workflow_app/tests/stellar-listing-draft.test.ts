// SPEC/MKT-STELLAR-DRAFT-01 — the fence the intake asked for.
//
// The packet's acceptance was previously exercised only by `tsc`, `next build` and a human looking at
// a connected DEV form: none of those can FAIL when the behaviour is wrong. This fence drives the
// claims directly — the mapping counts, the fifteen editable fields, the review items, every refusal
// rule, the pack carrying the listing values, and the adapter staying manual.
import assert from 'node:assert/strict'
import test from 'node:test'

import {
  STELLAR_DETAIL_FIELDS,
  stellarMapping,
  validateStellarDetails,
} from '@/lib/syndication/stellar-fields'
import { buildResoPropertyPayload, stellarTransportPlan } from '@/lib/syndication/stellar'
import type { ListingSource, StellarDetails } from '@/lib/syndication/types'

const emptySource: ListingSource = {
  id: 'prop-1',
  name: 'Casa Luar',
  slug: 'casa-luar',
  status: 'active',
  isPublished: false,
  listPrice: null,
  location: null,
  city: null,
  neighborhood: null,
  bedrooms: null,
  bathrooms: null,
  squareFeet: null,
  propertyType: null,
  shortDescription: null,
  publicRemarks: null,
  listingAgentName: null,
  listingAgentPhone: null,
  listingAgentEmail: null,
  publicUrl: null,
  heroMediaId: null,
  imageCount: 0,
  photos: [],
}

const fullDetails: StellarDetails = {
  listingContractDate: '2026-09-01',
  expirationDate: '2027-09-01',
  listingType: 'Exclusive right to sell',
  agentMlsId: 'PR-12345',
  taxId: '123-456-789',
  taxYear: 2026,
  annualTax: 4321.5,
  legalDescription: 'Lot 4, Culebra Ward',
  zoning: 'R-1',
  totalAreaSqft: 5000,
  heatedAreaSource: 'Appraiser',
  ownershipType: 'Fee simple',
  hoaDetails: 'None',
  showingInstructions: 'Call listing agent',
  occupantType: 'Vacant',
}

const fullSource: ListingSource = {
  ...emptySource,
  stellar: fullDetails,
  listPrice: 2700000,
  city: 'Culebra',
  bedrooms: 4,
  bathrooms: 3.5,
  bathroomsFull: 3,
  bathroomsHalf: 1,
  squareFeet: 3200,
  propertyType: 'residential',
  publicRemarks: 'Beachfront estate',
  listingAgentName: 'Listing Agent',
  streetNumber: '1',
  streetName: 'Culebra Esplanade',
  unitNumber: '2',
  stateOrProvince: 'PR',
  lotSize: 2,
  lotSizeUnits: 'Acres',
  yearBuilt: 2019,
  postalCode: '00775',
  isPublished: true,
  imageCount: 12,
}

test('stellar-draft: the counts are DERIVED and true, so a new field cannot leave them lying', () => {
  const mapping = stellarMapping(fullSource)
  assert.equal(mapping.counts.direct, 10)
  assert.equal(mapping.counts.converted, 7)
  assert.equal(mapping.counts.additional, 15)
  // The drift guard: the claim must equal what the maps actually contain.
  assert.equal(mapping.counts.direct, Object.keys(mapping.direct).length)
  assert.equal(mapping.counts.converted, Object.keys(mapping.converted).length)
  assert.equal(mapping.counts.additional, Object.keys(mapping.additional).length)
  assert.equal(mapping.counts.additional, STELLAR_DETAIL_FIELDS.length)
})

test('stellar-draft: an empty property reports every one of the fifteen editable fields as missing', () => {
  const mapping = stellarMapping(emptySource)
  for (const [, label] of STELLAR_DETAIL_FIELDS) {
    assert.ok(mapping.missing.includes(label), `the empty property should report "${label}" as missing`)
  }
  // 10 direct + 7 review + 15 editable — the whole face of the draft.
  assert.equal(mapping.missing.length, 32)
})


test('stellar-draft: a filled property has nothing missing, and empties are marked for REVIEW', () => {
  assert.deepEqual(stellarMapping(fullSource).missing, [])

  // THE SQUARE-FOOTAGE DECISION, asserted so a future edit cannot silently reverse it: on Culebra the
  // interior square footage IS the heated area, so it is SOURCED from the property rather than flagged
  // for review. (In a northern market the flag would be required — "heated" there excludes
  // unconditioned space.) The reason is recorded on the column itself: migration 195.
  const mapping = stellarMapping(fullSource)
  assert.equal(mapping.converted.HeatedAreaSqFt, fullSource.squareFeet)
  assert.equal(
    mapping.missing.includes('HeatedAreaSqFt (review)'),
    false,
    'interior footage is the heated area in this market, so it must not be flagged for review',
  )

  // A source missing only the values that need a unit check or a human decision: those come back
  // suffixed, because "empty" and "needs review" are different instructions to the operator.
  const partial = stellarMapping({ ...fullSource, bathroomsHalf: null, lotSizeUnits: null })
  assert.ok(partial.missing.includes('BathroomsHalf (review)'))
  assert.ok(partial.missing.includes('LotSize (review)'))
})

test('stellar-draft: every refusal rule rejects, with the message the form shows', () => {
  const valid = validateStellarDetails({
    listingContractDate: '2026-09-01',
    expirationDate: '2027-09-01',
    taxYear: '2026',
    annualTax: '4321.50',
    totalAreaSqft: '5000',
  })
  assert.equal(valid.ok, true)
  assert.equal(valid.ok && valid.values.taxYear, 2026)
  assert.equal(valid.ok && valid.values.annualTax, 4321.5)

  const badDate = validateStellarDetails({ listingContractDate: '09/01/2026' })
  assert.equal(badDate.ok, false)
  assert.match(badDate.ok === false ? badDate.error : '', /^Check listingContractDate/)

  assert.equal(validateStellarDetails({ annualTax: '-1' }).ok, false, 'negative tax must be refused')
  assert.equal(validateStellarDetails({ taxYear: '1700' }).ok, false, 'tax year range must be refused')
  assert.equal(validateStellarDetails({ taxYear: '2026.5' }).ok, false, 'fractional tax year must be refused')
  assert.equal(validateStellarDetails({ totalAreaSqft: '0' }).ok, false, 'zero area must be refused')

  const backwards = validateStellarDetails({
    listingContractDate: '2026-09-01',
    expirationDate: '2026-08-01',
  })
  assert.equal(backwards.ok, false)
  assert.equal(
    backwards.ok === false ? backwards.error : '',
    'Expiration must be after the contract date.',
  )
})

test('stellar-draft: the RESO pack carries the listing details, so the next pack includes them', () => {
  const payload = buildResoPropertyPayload(fullSource) as Record<string, unknown>
  assert.equal(payload.ExpirationDate, '2027-09-01')
  assert.equal(payload.ListAgentMlsId, 'PR-12345')
  assert.equal(payload.ParcelNumber, '123-456-789')
  assert.equal(payload.TaxYear, 2026)
  assert.equal(payload.TaxAnnualAmount, 4321.5)
  assert.equal(payload.ListingContractDate, '2026-09-01')
  // Nothing is claimed that the property does not have.
  const bare = buildResoPropertyPayload(emptySource) as Record<string, unknown>
  assert.equal(bare.ListAgentMlsId ?? null, null)
})

test('stellar-draft: the adapter stays MANUAL and never claims an accepted listing', () => {
  const plan = stellarTransportPlan(fullSource)
  assert.equal(plan.method, 'MANUAL')
  assert.equal(plan.dryRun, true)
  assert.equal(plan.liveEnabled, false)
  assert.equal(plan.kind, 'stellar.matrix_checklist')
  const cannotDo = plan.payload.cannotDo as string[]
  assert.ok(cannotDo.some((line) => /accepted MLS listing/i.test(line)))
  assert.ok(cannotDo.some((line) => /SkySlope Forms or Matrix/i.test(line)))
})
