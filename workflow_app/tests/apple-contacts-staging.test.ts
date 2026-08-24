import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// Apple Contacts — ODS client-priming V1 targeted tests (proofs 1-9).

import {
  appleContactToBatchItem,
  type AppleContactExport,
} from '../../lib/intake/apple-contacts'
import { lowerBatchItemToIntakeMessage } from '../../lib/intake/batch'
import { intakeSourceIdentity } from '../../lib/intake/contracts'
import {
  batchReplayDecision,
  decideStagingOutcome,
  normalizeProfile,
  profileFingerprint,
} from '../../scripts/load-apple-contacts'

const ACCOUNT = 'culebraluxe-lisa-icloud-contacts'

function contact(overrides: Partial<AppleContactExport> = {}): AppleContactExport {
  return {
    sourceId: 'CNCONTACT-1',
    namePrefix: '',
    givenName: 'Ada',
    middleName: '',
    familyName: 'Lovelace',
    nameSuffix: '',
    nickname: '',
    organization: '',
    department: '',
    jobTitle: '',
    emails: [],
    phones: [],
    postalAddresses: [],
    ...overrides,
  }
}

// Proof 1: sourceAccount is present in the canonical intake identity.
test('1. sourceAccount is present in the canonical intake identity', () => {
  const manifest = {
    importId: 'b1',
    sourceSystem: 'apple_contacts',
    adapter: 'apple-contacts.swift-json',
    adapterVersion: '1.0.0',
    importedAt: '2026-08-24T12:00:00.000Z',
    sourceAccount: ACCOUNT,
  }
  const lowered = lowerBatchItemToIntakeMessage(
    manifest,
    appleContactToBatchItem(contact(), 'b1', '2026-08-24T12:00:00.000Z'),
  )
  assert.equal(lowered.source.account, ACCOUNT)
  assert.equal(intakeSourceIdentity(lowered).sourceAccount, ACCOUNT)
  assert.equal(intakeSourceIdentity(lowered).source, 'apple_contacts')
  assert.equal(intakeSourceIdentity(lowered).externalEventId, 'CNCONTACT-1')
})

// Proof 2: complete multiple emails/phones/addresses survive normalization.
test('2. multiple emails / phones / postal addresses survive staging normalization', () => {
  const c = contact({
    emails: [
      { sourceLabel: 'home', value: 'home@x.com' },
      { sourceLabel: 'work', value: 'work@x.com' },
    ],
    phones: [
      { sourceLabel: 'mobile', value: '+1-555-0100' },
      { sourceLabel: 'home', value: '+1-555-0101' },
    ],
    postalAddresses: [
      { sourceLabel: 'home', street: '1 Main', city: 'Austin', state: 'TX', postalCode: '78701', country: 'USA', isoCountryCode: 'US' },
      { sourceLabel: 'work', street: '2 Oak', city: 'Round Rock', state: 'TX', postalCode: '78664', country: 'USA', isoCountryCode: 'US' },
    ],
  })
  const profile = normalizeProfile(c)
  assert.equal(profile.emails.length, 2)
  assert.equal(profile.phones.length, 2)
  assert.equal(profile.postalAddresses.length, 2)
  assert.deepEqual(profile.name, {
    prefix: '', given: 'Ada', middle: '', family: 'Lovelace', suffix: '', nickname: '',
  })
})

// Proof 3: exact replay creates no new profile revision.
test('3. same fingerprint -> exact replay, no new revision', () => {
  assert.equal(decideStagingOutcome({ payload_fingerprint: 'fp1' }, 'fp1'), 'replay')
})

// Proof 4: changed fingerprint creates the next revision.
test('4. different fingerprint -> changed revision', () => {
  assert.equal(decideStagingOutcome({ payload_fingerprint: 'fp1' }, 'fp2'), 'changed')
})

// Proof 5: same sourceId under two source accounts does not collide.
test('5. same sourceId under two source accounts does not collide', () => {
  const base = contact({ sourceId: 'CNCONTACT-SAME' })
  const mk = (account: string) =>
    lowerBatchItemToIntakeMessage(
      { importId: 'b1', sourceSystem: 'apple_contacts', adapter: 'a', adapterVersion: '1', importedAt: '2026-08-24T12:00:00.000Z', sourceAccount: account },
      appleContactToBatchItem(base, 'b1', '2026-08-24T12:00:00.000Z'),
    )
  const idA = intakeSourceIdentity(mk('account-a'))
  const idB = intakeSourceIdentity(mk('account-b'))
  assert.equal(idA.externalEventId, idB.externalEventId) // same sourceId
  assert.notEqual(idA.sourceAccount, idB.sourceAccount) // distinct account -> distinct durable key
})

// Proof 6: same batch id with a different checksum conflicts.
test('6. same batch id + different checksum is a truthful conflict', () => {
  assert.equal(batchReplayDecision({ file_sha256: 'AAAA' }, 'BBBB'), 'conflict')
  assert.equal(batchReplayDecision({ file_sha256: 'AAAA' }, 'AAAA'), 'replay')
  assert.equal(batchReplayDecision(undefined, 'AAAA'), 'new')
})

// Proof 7: batch totals balance (migration CHECK constraint present).
test('7. integration_intake_batch enforces a balance CHECK constraint', () => {
  const sql = readFileSync('db/migrations/072_integration_staged_contact_profile.sql', 'utf8')
  assert.match(sql, /integration_intake_batch_balance/)
  assert.match(sql, /input_count = valid_count \+ error_count/)
  assert.match(sql, /valid_count = new_profile_count \+ replay_count \+ changed_revision_count/)
})

// Proof 8: the loader never writes canonical Person/Client/business rows.
test('8. the loader performs no canonical Person/Client/interaction/Deal/workflow/task/event/outbox write', () => {
  const loader = readFileSync('scripts/load-apple-contacts.ts', 'utf8').toLowerCase()
  const forbidden = [
    'insert into person',
    'insert into client',
    'insert into interaction',
    'insert into deal',
    'insert into workflow',
    'insert into task',
    'insert into outbox_message',
  ]
  for (const f of forbidden) {
    assert.equal(loader.includes(f), false, `loader must not ${f}`)
  }
})

// Proof 9: contacts-export.json is ignored by Git.
test('9. contacts-export.json is gitignored', () => {
  const gitignore = readFileSync('.gitignore', 'utf8')
  assert.match(gitignore, /contacts-export\.json/)
})

// Fingerprint determinism over the normalized complete profile.
test('fingerprint is deterministic over the normalized profile', () => {
  const c = contact({ emails: [{ sourceLabel: 'work', value: 'a@x.com' }], givenName: 'Ada' })
  assert.equal(
    profileFingerprint(normalizeProfile(c)),
    profileFingerprint(normalizeProfile(c)),
  )
})
