import { test } from 'node:test'
import assert from 'node:assert/strict'

import { planSourceLinkedPersonConsolidations } from '../../lib/relationship-intel/person-consolidation-plan'

test('authoritative Apple profile consolidates legacy phone/email fragments into one Person', () => {
  const plan = planSourceLinkedPersonConsolidations(
    [
      {
        sourceProfileKey: 'apple_contacts\u0000local\u0000alicia-contact',
        survivorPersonId: 'person-alicia',
        identityKeys: [
          'email:alicia.geigel@gmail.com',
          'email:alicia.geigel@yahoo.com',
          'phone:7274201806',
        ],
      },
    ],
    [
      {
        personId: 'person-alicia',
        identityKeys: ['email:alicia.geigel@gmail.com'],
      },
      {
        personId: 'legacy-phone-person',
        identityKeys: ['phone:7274201806'],
      },
      {
        personId: 'legacy-yahoo-person',
        identityKeys: ['email:alicia.geigel@yahoo.com'],
      },
    ],
  )

  assert.deepEqual(
    plan.consolidations.map(({ survivorPersonId, loserPersonId }) => ({ survivorPersonId, loserPersonId })),
    [
      { survivorPersonId: 'person-alicia', loserPersonId: 'legacy-phone-person' },
      { survivorPersonId: 'person-alicia', loserPersonId: 'legacy-yahoo-person' },
    ],
  )
  assert.deepEqual(plan.skippedMultiWinnerLosers, [])
  assert.deepEqual(plan.skippedPartialIdentityLosers, [])
})

test('repair fails closed when a candidate Person owns identity outside the source profile', () => {
  const plan = planSourceLinkedPersonConsolidations(
    [
      {
        sourceProfileKey: 'apple_contacts\u0000local\u0000contact-a',
        survivorPersonId: 'person-a',
        identityKeys: ['phone:7875550101'],
      },
    ],
    [
      { personId: 'person-a', identityKeys: ['email:a@example.com'] },
      {
        personId: 'possible-other-human',
        identityKeys: ['phone:7875550101', 'email:other@example.com'],
      },
    ],
  )

  assert.deepEqual(plan.consolidations, [])
  assert.deepEqual(plan.skippedPartialIdentityLosers, ['possible-other-human'])
})

test('repair refuses a loser proposed for two different authoritative survivors', () => {
  const plan = planSourceLinkedPersonConsolidations(
    [
      {
        sourceProfileKey: 'apple_contacts\u0000local\u0000contact-a',
        survivorPersonId: 'person-a',
        identityKeys: ['phone:7875550101'],
      },
      {
        sourceProfileKey: 'apple_contacts\u0000local\u0000contact-b',
        survivorPersonId: 'person-b',
        identityKeys: ['phone:7875550101'],
      },
    ],
    [
      { personId: 'person-a', identityKeys: ['email:a@example.com'] },
      { personId: 'person-b', identityKeys: ['email:b@example.com'] },
      { personId: 'legacy', identityKeys: ['phone:7875550101'] },
    ],
  )

  assert.deepEqual(plan.consolidations, [])
  assert.deepEqual(plan.skippedMultiWinnerLosers, ['legacy'])
})
