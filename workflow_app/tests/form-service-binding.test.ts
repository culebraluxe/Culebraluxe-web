import { test } from 'node:test'
import assert from 'node:assert/strict'

import { mapOfferContractDraft } from '../../lib/forms/offer-contract-mapping'
import { mapShowingReportDraft } from '../../lib/forms/showing-report-mapping'
import { getActiveTemplate, getTemplate } from '../../lib/forms/template-registry'

test('FORM-SERVICE-CUT-01: SHOW-RPT maps Person + Property + report facts without Deal', () => {
  const mapped = mapShowingReportDraft(
    {
      id: 'form-show',
      personId: 'person-1',
      propertyId: 'property-1',
      fieldValues: {
        showingDate: '2026-09-06',
        duration: '45 minutes',
        outcome: 'Second showing',
        feedbackScore: '4',
      },
      sections: {
        feedback: 'Loved the view; kitchen is the only concern.',
        followUp: 'Send carrying costs and schedule Tuesday.',
      },
    },
    'showing-1',
  )

  assert.equal(mapped.showingId, 'showing-1')
  assert.equal(mapped.personId, 'person-1')
  assert.equal(mapped.propertyId, 'property-1')
  assert.equal(mapped.showingDate, '2026-09-06')
  assert.equal(mapped.outcome, 'Second showing')
  assert.equal(mapped.interestScore, 4)
  assert.match(mapped.feedback ?? '', /Loved the view/)
  assert.match(mapped.followUp ?? '', /Tuesday/)
  assert.ok(!('dealId' in mapped))
})

test('FORM-SERVICE-CUT-01: SHOW-RPT rejects invalid interest instead of inventing a score', () => {
  assert.throws(
    () =>
      mapShowingReportDraft(
        {
          id: 'form-show',
          personId: 'person-1',
          propertyId: 'property-1',
          fieldValues: { showingDate: '2026-09-06', feedbackScore: '9' },
          sections: {},
        },
        'showing-1',
      ),
    /1 to 5/,
  )
})

test('FORM-SERVICE-CUT-01: Offer Letter maps Person + Property into Contract-owned offer facts', () => {
  const mapped = mapOfferContractDraft({
    id: 'form-offer',
    personId: 'buyer-1',
    propertyId: 'property-1',
    fieldValues: {
      buyerName: 'Buyer One',
      sellerName: 'Seller One',
      brokerName: 'Lisa Penfield',
      property: 'Sea to Soul',
      offerAmount: '625000',
      deposit: '25000',
      financing: 'Cash',
      closingDate: '2026-10-15',
      expiration: '2026-09-10',
      contingencies: 'Inspection',
    },
    sections: { specialTerms: 'Furniture list to be agreed.' },
  })

  assert.equal(mapped.contractType, 'offer_letter')
  assert.equal(mapped.propertyId, 'property-1')
  assert.equal(mapped.roles.length, 1)
  assert.deepEqual(mapped.roles[0], {
    kind: 'person',
    personId: 'buyer-1',
    roleCode: 'BUYER',
    snapshotName: 'Buyer One',
  })
  assert.equal(mapped.facts.offerAmount, '625000')
  assert.equal(mapped.facts.financing, 'Cash')
  assert.equal(mapped.facts.specialTerms, 'Furniture list to be agreed.')
  assert.ok(!('dealId' in mapped))
})

test('FORM-SERVICE-CUT-01: OFFER-01 v2 is active while v1 remains addressable', () => {
  assert.equal(getActiveTemplate('OFFER-01')?.version, 2)
  assert.equal(getTemplate('OFFER-01', 1)?.version, 1)
  assert.equal(
    getTemplate('OFFER-01', 2)?.fields.find((field) => field.name === 'buyerName')?.binding,
    'person.displayName',
  )
  assert.equal(
    getTemplate('OFFER-01', 2)?.fields.find((field) => field.name === 'property')?.binding,
    'property.name',
  )
  assert.equal(
    getTemplate('OFFER-01', 2)?.fields.find((field) => field.name === 'offerAmount')?.binding,
    undefined,
  )
})
