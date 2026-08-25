import { test } from 'node:test'
import assert from 'node:assert/strict'

import { projectApplePersonToEvidence } from '../../lib/relationship-intel/apple-projector'

// ---------------------------------------------------------------------------
// REL-INTEL — Apple load projection into the neutral evidence seam.
// No database.
// ---------------------------------------------------------------------------

function applePerson() {
  return {
    id: 'lp-1',
    sourceAccount: 'culebraluxe-lisa-icloud-contacts',
    sourceContactId: 'CONTACT-ABC:ABPerson',
    displayName: 'Jane Q. Doe',
    organization: 'Casa Luar Management',
    emails: [
      { value: 'Jane.Doe@Example.com', normalized: 'jane.doe@example.com', label: '_$!<Work>!$_' },
      { value: 'Jane@Example.com', normalized: 'jane@example.com', label: null },
    ],
    phones: [
      { value: '+1 (787) 555-0134', normalized: '7875550134', label: '_$!<Mobile>!$_' },
    ],
  }
}

test('REL-INTEL: Apple person projects to neutral evidence preserving labels and emails', () => {
  const { evidence } = projectApplePersonToEvidence(applePerson())
  assert.equal(evidence.source, 'apple_contacts')
  assert.equal(evidence.sourceIdentityKey, 'CONTACT-ABC:ABPerson')
  assert.equal(evidence.displayName, 'Jane Q. Doe')
  assert.equal(evidence.organization, 'Casa Luar Management')
  assert.equal(evidence.hasEmail, true)
  assert.equal(evidence.hasPhone, true)
  assert.equal(evidence.knownAppleContact, true)
  assert.equal(evidence.emails.length, 2)
  assert.equal(evidence.phones.length, 1)
  assert.equal(evidence.phones[0].normalized, '7875550134')
})

test('REL-INTEL: Apple projection does not fabricate communication evidence', () => {
  const { evidence } = projectApplePersonToEvidence(applePerson())
  assert.equal(evidence.firstObservedAt, null)
  assert.equal(evidence.lastObservedAt, null)
  assert.equal(evidence.isTwoWay, null)
})

test('REL-INTEL: Apple projection fingerprint is deterministic and replay-stable', () => {
  const a = projectApplePersonToEvidence(applePerson())
  const b = projectApplePersonToEvidence(applePerson())
  assert.equal(a.fingerprint, b.fingerprint)
})
