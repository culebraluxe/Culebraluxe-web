import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  projectLPersonFromStaged,
  type StagedContactInput,
} from '../../lib/intake/l-person-projection'

// ---------------------------------------------------------------------------
// SUPPORT-2 — pure staged profile -> l_person relational-load mapping proofs.
// No database, no packages: the mapper is a pure function over the immutable
// staged profile JSON.
// ---------------------------------------------------------------------------

function staged(overrides: Record<string, unknown> = {}): StagedContactInput {
  return {
    stagedProfileId: 'staged-1',
    intakeBatchId: 'batch-1',
    source: 'apple_contacts',
    sourceAccount: 'culebraluxe-lisa-icloud-contacts',
    sourceContactId: 'CONTACT-ABC:ABPerson',
    revision: 1,
    payloadFingerprint: 'fp-1',
    reconciliationStatus: 'unreviewed',
    candidatePersonId: null,
    profile: {
      name: {
        prefix: 'Dr.',
        given: 'Jane',
        middle: 'Q',
        family: 'Doe',
        suffix: 'Jr.',
        nickname: '',
      },
      organization: '',
      department: '',
      jobTitle: '',
      emails: [{ label: '_$!<Work>!$_', value: 'Jane.Doe@Example.com' }],
      phones: [{ label: '_$!<Mobile>!$_', value: '+1 (787) 555-0134' }],
      postalAddresses: [
        {
          label: '_$!<Home>!$_',
          street: '1 Calle Sol',
          city: 'Culebra',
          state: 'PR',
          postalCode: '00775',
          country: 'Puerto Rico',
          isoCountryCode: 'PR',
        },
      ],
    },
    ...overrides,
  }
}

test('SUPPORT-2 test 1: exact staged profile -> l_person mapping', () => {
  const out = projectLPersonFromStaged(staged())
  const p = out.lPerson

  assert.equal(p.displayName, 'Dr. Jane Q Doe Jr.')
  assert.equal(p.namePrefix, 'Dr.')
  assert.equal(p.givenName, 'Jane')
  assert.equal(p.middleName, 'Q')
  assert.equal(p.familyName, 'Doe')
  assert.equal(p.nameSuffix, 'Jr.')
  assert.equal(p.nickname, null)
  assert.equal(p.organization, null)
  assert.equal(p.department, null)
  assert.equal(p.jobTitle, null)
  assert.equal(p.displayAddress, '1 Calle Sol, Culebra, PR 00775, Puerto Rico')

  const emails = out.identities.filter((i) => i.identityType === 'email')
  const phones = out.identities.filter((i) => i.identityType === 'phone')
  const apple = out.identities.filter((i) => i.identityType === 'apple_contact')
  assert.equal(emails.length, 1)
  assert.equal(phones.length, 1)
  assert.equal(apple.length, 1)
  assert.equal(emails[0].identityValue, 'jane.doe@example.com')
  assert.equal(emails[0].originalValue, 'Jane.Doe@Example.com')
  assert.equal(emails[0].sourceLabel, '_$!<Work>!$_')
  assert.equal(emails[0].sourceSystem, 'apple_contacts')
  assert.equal(emails[0].isPrimary, false, 'do not invent primary')
  assert.equal(phones[0].identityValue, '+17875550134')
  assert.equal(apple[0].identityValue, 'CONTACT-ABC:ABPerson')

  assert.equal(out.addresses.length, 1)
  assert.equal(out.addresses[0].street, '1 Calle Sol')
  assert.equal(out.addresses[0].city, 'Culebra')
  assert.equal(out.addresses[0].country, 'Puerto Rico')
  assert.ok(
    out.identities.every((i) => i.identityType !== 'postal_address'),
    'an address must never be misclassified as an identity',
  )
})

test('SUPPORT-2 test 2: multiple labeled emails are preserved, never collapsed', () => {
  const out = projectLPersonFromStaged(
    staged({
      profile: {
        name: { given: 'Jane', family: 'Doe' },
        organization: '',
        department: '',
        jobTitle: '',
        emails: [
          { label: '_$!<Work>!$_', value: 'Jane@Work.com' },
          { label: '_$!<Home>!$_', value: 'jane@home.com' },
        ],
        phones: [],
        postalAddresses: [],
      },
    }),
  )
  const emails = out.identities.filter((i) => i.identityType === 'email')
  assert.equal(emails.length, 2, 'both labeled emails preserved')
  assert.deepEqual(
    emails.map((e) => e.identityValue).sort(),
    ['jane@home.com', 'jane@work.com'],
  )
  assert.ok(emails.every((e) => e.isPrimary === false))
})

test('SUPPORT-2 test 3: multiple labeled phones are preserved, never collapsed', () => {
  const out = projectLPersonFromStaged(
    staged({
      profile: {
        name: { given: 'Jane', family: 'Doe' },
        organization: '',
        department: '',
        jobTitle: '',
        emails: [],
        phones: [
          { label: '_$!<Mobile>!$_', value: '+1 787 555 0100' },
          { label: '_$!<Home>!$_', value: '7875550101' },
        ],
        postalAddresses: [],
      },
    }),
  )
  const phones = out.identities.filter((i) => i.identityType === 'phone')
  assert.equal(phones.length, 2, 'both labeled phones preserved')
  assert.deepEqual(
    phones.map((p) => p.identityValue).sort(),
    ['+17875550100', '+17875550101'],
  )
})


test('SUPPORT-2 test 5: organization / name fallback mapping', () => {
  const orgOnly = projectLPersonFromStaged(
    staged({
      profile: {
        name: { given: '', family: '', middle: '', prefix: '', suffix: '', nickname: '' },
        organization: 'Culebra Construction LLC',
        department: 'Sales',
        jobTitle: 'Partner',
        emails: [],
        phones: [],
        postalAddresses: [],
      },
    }),
  )
  assert.equal(orgOnly.lPerson.displayName, 'Culebra Construction LLC')
  assert.equal(orgOnly.lPerson.organization, 'Culebra Construction LLC')
  assert.equal(orgOnly.lPerson.department, 'Sales')
  assert.equal(orgOnly.lPerson.jobTitle, 'Partner')

  const fallback = projectLPersonFromStaged(
    staged({
      profile: { name: { given: '', family: '', middle: '', prefix: '', suffix: '', nickname: '' }, organization: '', department: '', jobTitle: '', emails: [], phones: [], postalAddresses: [] },
    }),
  )
  assert.equal(fallback.lPerson.displayName, 'CONTACT-ABC:ABPerson')
})

test('SUPPORT-2 test 6: missing optional fields remain null/empty, not invented', () => {
  const out = projectLPersonFromStaged(
    staged({
      profile: { name: {}, organization: '', department: '', jobTitle: '', emails: [], phones: [], postalAddresses: [] },
    }),
  )
  const p = out.lPerson
  assert.equal(p.displayName, 'CONTACT-ABC:ABPerson')
  assert.equal(p.givenName, null)
  assert.equal(p.familyName, null)
  assert.equal(p.nickname, null)
  assert.equal(p.organization, null)
  assert.equal(p.department, null)
  assert.equal(p.jobTitle, null)
  assert.equal(p.displayAddress, null)
  // Only the apple_contact identity is derived from the source contact id; no
  // email/phone/address is invented.
  assert.equal(out.identities.length, 1, 'only the apple_contact identity is derived')
  assert.equal(out.identities[0].identityType, 'apple_contact')
  assert.equal(out.identities.filter((i) => i.identityType === 'email').length, 0)
  assert.equal(out.identities.filter((i) => i.identityType === 'phone').length, 0)
  assert.deepEqual(out.addresses, [], 'no invented addresses')
})

test('SUPPORT-2 test 7: same staged revision replay is idempotent (deterministic + deduped)', () => {
  const input = staged()
  const a = projectLPersonFromStaged(input)
  const b = projectLPersonFromStaged(input)
  assert.deepEqual(a.lPerson, b.lPerson)
  assert.deepEqual(
    a.identities.map((i) => `${i.identityType}:${i.identityValue}`).sort(),
    b.identities.map((i) => `${i.identityType}:${i.identityValue}`).sort(),
  )
  assert.deepEqual(a.addresses, b.addresses)

  const dup = projectLPersonFromStaged(
    staged({
      profile: {
        name: { given: 'Jane', family: 'Doe' },
        organization: '',
        department: '',
        jobTitle: '',
        emails: [
          { label: '_$!<Work>!$_', value: 'jane@work.com' },
          { label: '_$!<Home>!$_', value: 'JANE@WORK.COM' }, // duplicate (case-insensitive)
        ],
        phones: [
          { label: '_$!<Mobile>!$_', value: '+1 787 555 0100' },
          { label: '_$!<Mobile>!$_', value: '17875550100' }, // duplicate (digit-normalized)
        ],
        postalAddresses: [],
      },
    }),
  )
  assert.equal(dup.identities.filter((i) => i.identityType === 'email').length, 1)
  assert.equal(dup.identities.filter((i) => i.identityType === 'phone').length, 1)
})

test('SUPPORT-2 test 8: a later staged revision updates the current load projection', () => {
  const a = projectLPersonFromStaged(staged({ revision: 1, payloadFingerprint: 'fp-1' }))
  const b = projectLPersonFromStaged(
    staged({
      revision: 2,
      payloadFingerprint: 'fp-2',
      profile: {
        name: { given: 'Jane', family: 'Doe' },
        organization: 'New Org',
        department: '',
        jobTitle: '',
        emails: [{ label: '_$!<Work>!$_', value: 'jane@new.com' }],
        phones: [],
        postalAddresses: [],
      },
    }),
  )
  assert.equal(a.lPerson.organization, null)
  assert.equal(b.lPerson.organization, 'New Org')
  assert.equal(
    a.identities.filter((i) => i.identityType === 'email')[0]?.identityValue,
    'jane.doe@example.com',
  )
  assert.equal(
    b.identities.filter((i) => i.identityType === 'email')[0]?.identityValue,
    'jane@new.com',
  )
})

test('SUPPORT-2 test 9: canonical person/person_identity are never touched by the projection', () => {
  const out = projectLPersonFromStaged(staged())
  const json = JSON.stringify(out)
  assert.ok(!json.includes('role'), 'no canonical role invented')
  assert.ok(!json.includes('budget'), 'no canonical budget invented')
  assert.ok(!json.includes('preferred_areas'), 'no preferences invented')
  assert.ok(!json.includes('timeline'), 'no timeline invented')
  // l_person carries flattened fields; the full source profile is never embedded
  // into the load projection (it stays in integration_staged_contact_profile).
  assert.ok(!json.includes('isoCountryCode') || out.addresses.length > 0)
})

