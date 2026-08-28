import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { IssuedExecutionSlot } from '../../lib/agreements/execution'
import { resolveSignatureEnvelopeRecipients } from '../../lib/forms/signature-envelope'

const lisa: IssuedExecutionSlot = {
  slotId: 'SELLER_BROKER:1',
  role: 'SELLER_BROKER',
  personId: 'lisa',
  name: 'Lisa Penfield',
  email: 'lisa@culebraluxe.com',
  required: true,
  order: 0,
}

function external(index: number): IssuedExecutionSlot {
  return {
    slotId: `BUYER:${index}`,
    role: 'BUYER',
    personId: `party-${index}`,
    name: `Party ${index}`,
    email: `party${index}@example.com`,
    required: true,
    order: index,
  }
}

for (const count of [1, 2, 4]) {
  test(`one envelope: Lisa + ${count} external ${count === 1 ? 'party' : 'parties'}`, () => {
    const result = resolveSignatureEnvelopeRecipients(
      [lisa, ...Array.from({ length: count }, (_, index) => external(index + 1))],
      [lisa.slotId],
    )
    assert.equal(result.ok, true)
    if (!result.ok) return
    assert.equal(result.recipients.length, count)
    assert.deepEqual(
      result.recipients.map((recipient) => recipient.order),
      Array.from({ length: count }, (_, index) => index + 1),
    )
    assert.deepEqual(
      result.recipients.map((recipient) => recipient.executionSlotId),
      Array.from({ length: count }, (_, index) => `BUYER:${index + 1}`),
    )
    assert.equal(
      result.recipients.some((recipient) => recipient.email === lisa.email),
      false,
      'Lisa is preserved in the PDF but never becomes a provider signer',
    )
  })
}

test('envelope resolution fails closed when any required external signer lacks email', () => {
  const result = resolveSignatureEnvelopeRecipients(
    [lisa, { ...external(1), email: null }],
    [lisa.slotId],
  )
  assert.deepEqual(result, {
    ok: false,
    error: "Execution slot 'BUYER:1' requires a signer name and email address.",
  })
})
