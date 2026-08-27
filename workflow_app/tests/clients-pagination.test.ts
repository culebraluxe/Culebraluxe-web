import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { getClientsPage } from '../../db/clients'
import { getClientAdminPage } from '../../db/client-admin'
import { getClientContactHistory } from '../../db/contact-history'
import {
  pickPrimaryIdentity,
  groupEvidenceForPromotion,
  displayNameForEvidence,
} from '../../db/promote-evidence'
import { isHumanName } from '../../lib/relationship-intel/names'
import {
  identityMatchKey,
  buildContactIndex,
  resolveContactForIdentityKeys,
} from '../../db/enrich-people'

// ---------------------------------------------------------------------------
// CLIENTS — server-side pagination over the canonical `person` parent (no
// full-table materialization) + the evidence -> canonical Person promotion
// helpers. DB-free where possible (fake QueryExecutor).
// ---------------------------------------------------------------------------

type Row = Record<string, any>

/** Fake tagged-template QueryExecutor that answers COUNT + page queries. */
function fakeExecute(pageRows: Row[], total: number, evidenceRows: Row[] = []) {
  const calls: Array<{ text: string; params: any[] }> = []
  const fn: any = (strings: TemplateStringsArray, ...params: any[]) => {
    const text = strings.join('__')
    calls.push({ text, params })
    if (text.includes('as total')) return [{ total }]
    if (text.includes('integration_relationship_evidence')) return evidenceRows
    return pageRows
  }
  ;(fn as any).calls = calls
  return fn
}

test('clients: getClientsPage issues COUNT + LIMIT/OFFSET page over the read model', async () => {
  const pageRows: Row[] = [
    {
      person_id: 'p1',
      display_name: 'Jane Doe',
      name_sort_priority: 1,
      role: 'buyer',
      status: 'new',
      location: 'Culebra',
      primary_email: 'jane@example.com',
      primary_phone: '+17875550134',
      assigned_agent: null,
      last_contact_label: 'Aug 20, 2026',
      sources: ['apple_messages'],
    },
  ]
  const execute = fakeExecute(pageRows, 137, [
    {
      id: 'e1',
      source: 'apple_messages',
      source_account: 'E:lisapenfield@icloud.com',
      source_identity_key: '+18609895020',
      source_label: 'iMessage',
      display_name: null,
      organization: null,
      emails: [],
      phones: [{ value: '+18609895020', normalized: '8609895020', label: null }],
      first_observed_at: null,
      last_observed_at: null,
      last_inbound_at: null,
      last_outbound_at: null,
      inbound_count: 2431,
      outbound_count: 2413,
      is_two_way: true,
      is_owner_initiated: null,
      is_automated_or_bulk: null,
      is_organization_or_service: null,
      known_apple_contact: false,
      has_email: false,
      has_phone: true,
      coverage_note: null,
      canonical_person_id: 'p1',
      match_method: 'exact_phone',
      match_confidence: 'exact',
      review_state: 'exact_linked',
      match_reason: 'promoted_new_identity',
      rule_version: 'rel-intel/v1',
      evidence_fingerprint: 'ami-proof',
      updated_at: '2026-08-25T11:50:37.299Z',
    },
  ])
  const result = await getClientsPage(
    { search: 'jane', status: 'active', role: 'buyer', sort: 'name', page: 3, pageSize: 50 },
    execute as never,
  )

  assert.equal(result.total, 137)
  assert.equal(result.page, 3)
  assert.equal(result.pageSize, 50)
  assert.equal(result.rows.length, 1)
  const row = result.rows[0]
  assert.equal(row.displayName, 'Jane Doe')
  assert.equal(row.primaryEmail, 'jane@example.com')
  assert.deepEqual(row.sources, ['apple_messages'])
  assert.equal(row.relationshipActivity.observedCommunicationCount, 4844)
  assert.equal(row.relationshipActivity.inboundCount, 2431)
  assert.equal(row.relationshipActivity.outboundCount, 2413)
  assert.equal(row.relationshipActivity.twoWay, true)
  assert.equal(row.relationshipActivity.lastObservedAt, null)

  // separate COUNT(*) and a page query both ran
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')), 'COUNT query present')
  // SQL LIMIT/OFFSET flow as top-level params (50/page; page 3 -> offset 100)
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'pageSize 50 reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.params.includes(100)), 'offset 100 on page 3')
})

test('clients: getClientsPage search/filter/sort/limit are SQL against the read model (source guard)', () => {
  const src = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/mv\.search_text ilike \$\{like\}/.test(src), 'search is a SQL ILIKE on the pre-shaped view')
  assert.ok(/from mv_client_directory mv/.test(src), 'reads from the materialized read model')
  assert.ok(/limit \$\{pageSize\} offset \$\{offset\}/.test(src), 'SQL LIMIT/OFFSET')
  assert.ok(/count\(\*\)::int as total/.test(src), 'separate COUNT')
  assert.ok(/and \(\$\{status\}::text is null or mv\.status = \$\{status\}\)/.test(src), 'status filter in SQL')
  assert.ok(/order by \$\{ORDER_FRAGMENTS\[sort\]\}/.test(src), 'sort is a SQL ORDER BY')
})

test('clients: getClientAdminPage issues COUNT + LIMIT/OFFSET over person', async () => {
  const pageRows: Row[] = [
    {
      id: 'p1',
      display_name: 'Jane Doe',
      role: 'buyer',
      status: 'new',
      location: null,
      assigned_agent: null,
      primary_email: 'jane@example.com',
      primary_phone: null,
      last_interaction_label: null,
      open_task_count: 0,
      active_deal_count: 0,
      interest_count: 0,
    },
  ]
  const execute = fakeExecute(pageRows, 87)
  const result = await getClientAdminPage({ search: 'jane', page: 2, pageSize: 50 }, execute as never)
  assert.equal(result.total, 87)
  assert.equal(result.page, 2)
  assert.equal(result.rows.length, 1)
  assert.equal(result.rows[0].displayName, 'Jane Doe')
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')), 'COUNT query present')
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'pageSize 50 reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'offset 50 on page 2')
})

// --- evidence -> canonical Person promotion helpers (pure) ---

test('promote: pickPrimaryIdentity prefers phone, then email, else null', () => {
  const phone = pickPrimaryIdentity(
    [{ value: 'a@b.com', normalized: 'a@b.com' }],
    [{ value: '+17875550134', normalized: '7875550134' }],
  )
  assert.equal(phone.kind, 'phone')
  assert.equal(phone.value, '7875550134')

  const email = pickPrimaryIdentity(
    [{ value: 'Jane@Example.com', normalized: 'jane@example.com' }],
    [],
  )
  assert.equal(email.kind, 'email')
  assert.equal(email.value, 'jane@example.com')

  assert.equal(pickPrimaryIdentity([], []), null)
})

test('promote: groupEvidenceForPromotion dedupes by identity (SMS+iMessage collapse)', () => {
  const rows = [
    { id: 'e1', displayName: null, source: 'apple_messages', emails: [], phones: [{ value: '+17875550134', normalized: '7875550134' }] },
    { id: 'e2', displayName: null, source: 'apple_messages', emails: [], phones: [{ value: '17875550134', normalized: '7875550134' }] },
    { id: 'e3', displayName: 'Bob', source: 'gmail_contacts', emails: [{ value: 'b@c.com', normalized: 'b@c.com' }], phones: [] },
  ]
  const groups = groupEvidenceForPromotion(rows as any)
  assert.equal(groups.length, 2, 'one group per unique identity')
  const phoneGroup = groups.find((g) => g.identity.kind === 'phone')
  assert.equal(phoneGroup!.evidenceIds.length, 2, 'both phone evidence rows share one Person')
  const emailGroup = groups.find((g) => g.identity.kind === 'email')
  assert.equal(emailGroup!.evidenceIds.length, 1)
})

test('promote: displayNameForEvidence uses a real name, else the identity label', () => {
  const named = [
    { id: 'e1', displayName: 'Bob', source: 'gmail_contacts', emails: [{ value: 'b@c.com', normalized: 'b@c.com' }], phones: [] },
  ]
  const namedGroups = groupEvidenceForPromotion(named as any)
  assert.equal(displayNameForEvidence(named as any, namedGroups[0]), 'Bob')

  const unnamed = [
    { id: 'e1', displayName: null, source: 'apple_messages', emails: [], phones: [{ value: '+17875550134', normalized: '7875550134' }] },
  ]
  const unnamedGroups = groupEvidenceForPromotion(unnamed as any)
  assert.equal(displayNameForEvidence(unnamed as any, unnamedGroups[0]), '+17875550134')
})

// --- display-name classification (CORE: IDENTITY IS NOT DISPLAY NAME) ---

test('names: isHumanName accepts real names, rejects phone/email/structured IDs', () => {
  assert.equal(isHumanName('Dave Bills Friend'), true)
  assert.equal(isHumanName('Kavita'), true)
  assert.equal(isHumanName('+12013424797'), false)
  assert.equal(isHumanName('2013424797'), false)
  assert.equal(isHumanName('bob@example.com'), false)
  assert.equal(isHumanName('759147B4-9BF0-4C1D-8E26-280D79168D5F:ABPerson'), false)
  assert.equal(isHumanName(''), false)
  assert.equal(isHumanName(null), false)
})

// --- Apple Contacts name enrichment helpers (pure) ---

test('enrich: identityMatchKey normalizes phone/email to one stable key', () => {
  assert.equal(identityMatchKey('phone', '+1 (787) 555-0134'), 'phone:7875550134')
  assert.equal(identityMatchKey('phone', '7875550134'), 'phone:7875550134')
  assert.equal(identityMatchKey('email', 'Jane@Example.com'), 'email:jane@example.com')
  assert.equal(identityMatchKey('phone', ''), null)
})

test('enrich: buildContactIndex matches the same normalized identity and resolves the human-named contact', () => {
  const contacts = [
    { displayName: null, organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '+17875550134' },
    { displayName: 'Jane Doe', organization: 'Acme', displayAddress: '1 Calle Sol', identityType: 'phone', normalizedValue: '7875550134' },
  ]
  const index = buildContactIndex(contacts as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:7875550134'], index)
  assert.equal(ambiguous, false)
  assert.equal(contact?.displayName, 'Jane Doe')
  assert.equal(contact?.organization, 'Acme')
  assert.equal(contact?.displayAddress, '1 Calle Sol')
})

test('enrich: resolveContactForIdentityKeys is ambiguous (never guesses) when names conflict', () => {
  const contacts = [
    { displayName: 'Jane Doe', organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '7875550134' },
    { displayName: 'Jane Smith', organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '7875550134' },
  ]
  const index = buildContactIndex(contacts as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:7875550134'], index)
  assert.equal(ambiguous, true, 'two distinct names for one identity is ambiguous')
  assert.equal(contact, null)
})

test('enrich: resolveContactForIdentityKeys returns null when no identity matches', () => {
  const index = buildContactIndex([
    { displayName: 'Jane', organization: null, displayAddress: null, identityType: 'email', normalizedValue: 'jane@example.com' },
  ] as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:9999999999'], index)
  assert.equal(contact, null)
  assert.equal(ambiguous, false)
})

// --- name resolution flag on the directory projection ---

test('clients: getClientsPage marks nameResolved from the read-model sort priority', async () => {
  const pageRows: Row[] = [
    { person_id: 'p1', display_name: 'Jane Doe', name_sort_priority: 1, role: 'buyer', status: 'new', location: null, primary_email: null, primary_phone: null, assigned_agent: null, last_contact_label: null, sources: [] },
    { person_id: 'p2', display_name: '2039805771', name_sort_priority: 0, role: 'buyer', status: 'new', location: null, primary_email: null, primary_phone: '2039805771', assigned_agent: null, last_contact_label: null, sources: [] },
  ]
  const execute = fakeExecute(pageRows, 2)
  const result = await getClientsPage({ page: 1, pageSize: 50 }, execute as never)
  assert.equal(result.rows[0].nameResolved, true, 'human name is resolved')
  assert.equal(result.rows[1].nameResolved, false, 'phone fallback is unresolved')
})

// --- contact history (server-side paged, newest first) ---

test('clients: getClientContactHistory pages over the read model newest-first', async () => {
  const pageRows: Row[] = [
    { interaction_id: 'i1', channel: 'imessage', direction: 'outbound', occurred_at_label: 'Aug 25, 2026 09:00 AM', title: 'Re: showing', summary: 'Confirm 2pm' },
  ]
  const execute = fakeExecute(pageRows, 45)
  const result = await getClientContactHistory('person-1', { page: 2, pageSize: 20 }, execute as never)
  assert.equal(result.total, 45)
  assert.equal(result.page, 2)
  assert.equal(result.pageSize, 20)
  assert.equal(result.rows.length, 1)
  assert.equal(result.rows[0].channel, 'imessage')
  assert.equal(result.rows[0].direction, 'outbound')
  assert.equal(result.rows[0].title, 'Re: showing')
  assert.equal(result.rows[0].occurredAt, 'Aug 25, 2026 09:00 AM')
  assert.ok(execute.calls.some((c) => c.text.includes('as total')), 'COUNT query present')
  assert.ok(execute.calls.some((c) => c.params.includes(20)), 'pageSize 20 reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.params.includes(20)), 'offset 20 on page 2')
})

test('clients: getClientContactHistory is newest-first SQL against the read model (source guard)', () => {
  const src = readFileSync(new URL('../../db/contact-history.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_contact_history mv/.test(src), 'reads from the materialized read model')
  assert.ok(/order by mv\.occurred_at desc/.test(src), 'SQL ORDER BY occurred_at DESC')
  assert.ok(/limit \$\{pageSize\} offset \$\{offset\}/.test(src), 'SQL LIMIT/OFFSET')
  assert.ok(/count\(\*\)::int as total/.test(src), 'separate COUNT')
})

// --- resolved-first people sort + final geometry guards ---

test('clients: name sort uses the read-model resolved-first priority (source guard)', () => {
  const src = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/mv\.name_sort_priority desc/.test(src), 'resolved-first priority from the read model')
  assert.ok(/mv\.display_name asc, mv\.person_id asc/.test(src), 'then name asc, id tie-break')
  const migration = readFileSync(
    new URL('../../db/migrations/080_mv_client_read_models.sql', import.meta.url),
    'utf8',
  )
  assert.ok(/name_sort_priority/.test(migration), 'read model materializes a sort priority')
})

test('clients: the deleted small Last contact pane is absent from the working pane', () => {
  const src = readFileSync(
    new URL('../../components/portal/client-manager.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(!/heading="Last contact"/.test(src), 'no Last contact pane between Act and Interests')
})

test('clients: Contact History is navy, fills its row, scrolls internally, server-paged (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/contact-history.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/variant="feature"/.test(src), 'navy feature panel')
  assert.ok(/overflow-auto/.test(src), 'scrolls both axes (X and Y)')
  assert.ok(/min-w-\[34rem\]/.test(src), 'internal grid wider than the navy viewport (horizontal scroll)')
  assert.ok(/flex-1 overflow-auto/.test(src), 'fills the Client Card row and scrolls inside (never taller than the card)')
  assert.ok(/pageSize/.test(src), 'server-side paging')
})

test('clients: CORE Clients working pane is the balanced relationship workspace (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/client-manager.tsx', import.meta.url),
    'utf8',
  )
  // Reused shared Command + Status band, not a local copy.
  assert.ok(/CommandStatusBand/.test(src), 'reuses the shared Command + Status band')
  // The band uses the shared balanced (50/50) preset so the COMMAND | STATUS
  // seam lines up with the CLIENT CARD | CONTACT HISTORY row beneath it.
  assert.ok(/ratio="balanced"/.test(src), 'uses the shared balanced ratio preset')
  assert.ok(/lg:grid-cols-2 lg:gap-4/.test(src), 'working rows share the band gap for exact seam alignment')
  // Client Card + Contact History equal 50/50 rows (plus Interests + Notes).
  assert.ok(/ClientCard/.test(src), 'Client Card present')
  assert.ok(/lg:grid-cols-2/.test(src), 'equal-width 50/50 working rows')
  // Removed: Act manual-action grid, duplicate Timeline, Log-a-note mini-entry.
  assert.ok(!/heading="Act"/.test(src), 'Act panel removed')
  assert.ok(!/heading="Timeline"/.test(src), 'duplicate Timeline removed')
  assert.ok(!/heading="Log a note"/.test(src), 'mini interaction-entry removed')
  // Notes is a simple free-form + Save surface beside Interests.
  assert.ok(/ClientNotes/.test(src), 'simple free-form Notes present')
})

test('clients: directory/history reads come from materialized views, not L/ODS (source guard)', () => {
  const clients = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_directory mv/.test(clients), 'directory reads the read model')
  assert.ok(!/l_person|integration_relationship_evidence/.test(clients), 'no ODS reconstruction in the directory read')

  const history = readFileSync(new URL('../../db/contact-history.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_contact_history mv/.test(history), 'history reads the read model')
  assert.ok(!/from interaction\b/.test(history), 'no raw interaction reconstruction in the history read')
})

test('clients: refresh seam uses CONCURRENTLY (replay/refresh safe, unique index present)', () => {
  const src = readFileSync(new URL('../../db/client-read-models.ts', import.meta.url), 'utf8')
  assert.ok(/refresh materialized view concurrently/i.test(src), 'CONCURRENTLY refresh')
  const migration = readFileSync(
    new URL('../../db/migrations/080_mv_client_read_models.sql', import.meta.url),
    'utf8',
  )
  assert.ok(/create unique index mv_client_directory_pk/.test(migration), 'unique index for concurrent refresh')
})
