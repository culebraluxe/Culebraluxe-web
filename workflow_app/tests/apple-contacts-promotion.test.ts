import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { reconcileEvidence, type PersonLookup } from '../../lib/relationship-intel/reconcile'
import { projectApplePersonToEvidence } from '../../lib/relationship-intel/apple-projector'
import {
  groupEvidenceForPromotion,
  pickPrimaryIdentity,
  type PromotionEvidence,
} from '../../db/promote-evidence'

// ---------------------------------------------------------------------------
// Apple Contacts -> canonical Person lifecycle — targeted proofs (no PROD).
// ---------------------------------------------------------------------------

const NO_MATCH: PersonLookup = {
  findExplicitSourceLink: async () => null,
  findPeopleByEmail: async () => [],
  findPeopleByPhone: async () => [],
}

function emailLookup(personId: string): PersonLookup {
  return {
    findExplicitSourceLink: async () => null,
    findPeopleByEmail: async () => [{ personId }],
    findPeopleByPhone: async () => [],
  }
}

function ambiguousEmailLookup(a: string, b: string): PersonLookup {
  return {
    findExplicitSourceLink: async () => null,
    findPeopleByEmail: async () => [{ personId: a }, { personId: b }],
    findPeopleByPhone: async () => [],
  }
}

function phoneLookup(personId: string): PersonLookup {
  return {
    findExplicitSourceLink: async () => null,
    findPeopleByEmail: async () => [],
    findPeopleByPhone: async () => [{ personId }],
  }
}

function baseEvidence(overrides: Record<string, unknown> = {}) {
  return {
    source: 'apple_contacts',
    sourceAccount: 'culebraluxe-lisa-icloud-contacts',
    sourceIdentityKey: 'CONTACT-ABC:ABPerson',
    sourceLabel: null,
    displayName: 'Jessica Iverson',
    organization: null,
    emails: [{ value: 'jessica@bodysoulandbeauty.com', normalized: 'jessica@bodysoulandbeauty.com', label: null }],
    phones: [{ value: '+34689351739', normalized: '34689351739', label: null }],
    firstObservedAt: null,
    lastObservedAt: null,
    lastInboundAt: null,
    lastOutboundAt: null,
    inboundCount: null,
    outboundCount: null,
    isTwoWay: null,
    isOwnerInitiated: null,
    isAutomatedOrBulk: null,
    isOrganizationOrService: null,
    knownAppleContact: true,
    hasEmail: true,
    hasPhone: true,
    coverageNote: null,
    ...overrides,
  }
}

// --- Reconcile classification (drives what gets promoted) ------------------

test('promotion 1: unmatched valid Apple person classifies unmatched (-> promoted)', async () => {
  const d = await reconcileEvidence(baseEvidence() as never, NO_MATCH)
  assert.equal(d.reviewState, 'unmatched')
})

test('promotion 2: exact normalized email match reuses existing Person', async () => {
  const d = await reconcileEvidence(baseEvidence() as never, emailLookup('p-jessica'))
  assert.equal(d.reviewState, 'exact_linked')
  assert.equal(d.canonicalPersonId, 'p-jessica')
})

test('promotion 3: exact reliable normalized phone match reuses existing Person', async () => {
  const d = await reconcileEvidence(baseEvidence() as never, phoneLookup('p-jessica'))
  assert.equal(d.reviewState, 'exact_linked')
  assert.equal(d.canonicalPersonId, 'p-jessica')
})

test('promotion 4: ambiguous identity match -> no canonical Person (review)', async () => {
  const d = await reconcileEvidence(baseEvidence() as never, ambiguousEmailLookup('p-1', 'p-2'))
  assert.equal(d.reviewState, 'ambiguous')
  assert.equal(d.canonicalPersonId, null)
})

test('promotion 5: service/org-only Apple contact is never promoted', async () => {
  const d = await reconcileEvidence(
    baseEvidence({ isOrganizationOrService: true }) as never,
    NO_MATCH,
  )
  assert.equal(d.reviewState, 'non_person')
  assert.equal(d.canonicalPersonId, null)
  // The Apple projector flags org-only contacts (no person name, has org).
  const projected = projectApplePersonToEvidence({
    id: 'lp-1',
    sourceAccount: 'a',
    sourceContactId: 'C:1',
    displayName: 'Acme Corp',
    organization: 'Acme Corp',
    emails: [],
    phones: [],
    isOrganizationOrService: true,
  })
  assert.equal(projected.evidence.isOrganizationOrService, true)
})

// --- Dedupe / survivorship (replay cannot duplicate) -----------------------

test('promotion 6/7: replay dedupes by primary identity -> no duplicate Person/identity', () => {
  const rows: PromotionEvidence[] = [
    { id: 'e1', source: 'apple_contacts', displayName: 'Jessica Iverson', emails: [{ value: 'j@b.com', normalized: 'j@b.com' }], phones: [] },
    { id: 'e2', source: 'apple_contacts', displayName: 'Jessica Iverson', emails: [{ value: 'j@b.com', normalized: 'j@b.com' }], phones: [] },
  ]
  const groups = groupEvidenceForPromotion(rows)
  assert.equal(groups.length, 1, 'one identity -> one promotion group (one Person)')
  assert.equal(groups[0].evidenceIds.length, 2)
  assert.equal(pickPrimaryIdentity(rows[0].emails, rows[0].phones)?.value, 'j@b.com')
})

test('promotion 8: changed contact preserves survivorship (same identity reused)', () => {
  const before: PromotionEvidence[] = [
    { id: 'e1', source: 'apple_contacts', displayName: 'Jessica Iverson', emails: [{ value: 'j@b.com', normalized: 'j@b.com' }], phones: [] },
  ]
  const after: PromotionEvidence[] = [
    { id: 'e2', source: 'apple_contacts', displayName: 'Jessica Iverson', emails: [{ value: 'j@b.com', normalized: 'j@b.com' }], phones: [] },
  ]
  const g1 = groupEvidenceForPromotion(before)
  const g2 = groupEvidenceForPromotion(after)
  assert.equal(g1[0].key, g2[0].key, 'same identity key -> same canonical Person reused')
})

// --- Orchestrator: stages in order, MV refresh once, SUCCESS gating ---------

test('promotion 9: full orchestrator invokes projection then promotion after ODS staging', () => {
  const sh = readFileSync('scripts/contacts-sync.sh', 'utf8')
  const loadIdx = sh.indexOf('load-apple-contacts.ts')
  const projectIdx = sh.indexOf('project-apple-contacts.ts')
  const promoteIdx = sh.indexOf('promote-apple-contacts.ts')
  assert.ok(loadIdx >= 0 && projectIdx >= 0 && promoteIdx >= 0)
  assert.ok(loadIdx < projectIdx && projectIdx < promoteIdx, 'ODS -> projection -> promotion order')
})

test('promotion 10: successful promotion refreshes the client read model once', () => {
  const src = readFileSync('db/promote-evidence.ts', 'utf8')
  // promoteEvidence must call the shared refresh seam exactly once.
  assert.equal((src.match(/refreshClientReadModels\(\)/g) ?? []).length, 1)
})

test('promotion 11: failed downstream promotion prevents final SUCCESS', () => {
  const sh = readFileSync('scripts/contacts-sync.sh', 'utf8')
  const promoteIdx = sh.indexOf('promote-apple-contacts.ts')
  const successIdx = sh.indexOf('SUCCESS:')
  assert.ok(promoteIdx >= 0 && successIdx >= 0)
  assert.ok(successIdx > promoteIdx, 'SUCCESS is printed only after the promotion stage')
  assert.ok(sh.includes('fail "Contacts canonical promotion'), 'promotion failure fails the run')
})

// --- Clients UI: New Client entry removed, Edit preserved ------------------

test('promotion 12: New Client / Add Client button no longer renders', () => {
  const src = readFileSync('components/portal/client-manager.tsx', 'utf8')
  assert.ok(!/>\s*New\s*<\/button>/.test(src), 'no visible New button')
  assert.ok(!src.includes('setShowCreate(true)'), 'no create entry point in the manager')
})

test('promotion 13: Edit Client / create backend remains available', () => {
  const manager = readFileSync('components/portal/client-manager.tsx', 'utf8')
  const editor = readFileSync('components/portal/client-editor.tsx', 'utf8')
  assert.ok(manager.includes('showEdit'), 'edit path preserved')
  assert.ok(editor.includes('mode === "create"'), 'create editor mode preserved')
  assert.ok(editor.includes('createClientAction'), 'create server action preserved')
})

