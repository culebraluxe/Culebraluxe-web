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

type Row = Record<string, any>

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
  const pageRows: Row[] = [{
    person_id: 'p1', display_name: 'Jane Doe', name_sort_priority: 1, role: 'buyer', status: 'new',
    location: 'Culebra', primary_email: 'jane@example.com', primary_phone: '+17875550134',
    assigned_agent: null, last_contact_label: 'Aug 20, 2026', sources: ['apple_messages'],
  }]
  const execute = fakeExecute(pageRows, 137, [{
    id: 'e1', source: 'apple_messages', source_account: 'E:lisapenfield@icloud.com',
    source_identity_key: '+18609895020', source_label: 'iMessage', display_name: null,
    organization: null, emails: [], phones: [{ value: '+18609895020', normalized: '8609895020', label: null }],
    first_observed_at: null, last_observed_at: null, last_inbound_at: null, last_outbound_at: null,
    inbound_count: 2431, outbound_count: 2413, is_two_way: true, is_owner_initiated: null,
    is_automated_or_bulk: null, is_organization_or_service: null, known_apple_contact: false,
    has_email: false, has_phone: true, coverage_note: null, canonical_person_id: 'p1',
    match_method: 'exact_phone', match_confidence: 'exact', review_state: 'exact_linked',
    match_reason: 'promoted_new_identity', rule_version: 'rel-intel/v1', evidence_fingerprint: 'ami-proof',
    updated_at: '2026-08-25T11:50:37.299Z',
  }])
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
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')), 'COUNT query present')
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'pageSize 50 reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.params.includes(100)), 'offset 100 on page 3')
})

test('clients: getClientsPage search/filter/sort/limit are SQL against the read model (source guard)', () => {
  const src = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/mv\.search_text ilike \$\{like\}/.test(src))
  assert.ok(/from mv_client_directory mv/.test(src))
  assert.ok(/limit \$\{pageSize\} offset \$\{offset\}/.test(src))
  assert.ok(/count\(\*\)::int as total/.test(src))
  assert.ok(/and \(\$\{status\}::text is null or mv\.status = \$\{status\}\)/.test(src))
  assert.ok(/order by \$\{ORDER_FRAGMENTS\[sort\]\}/.test(src))
})

test('clients: getClientAdminPage issues COUNT + LIMIT/OFFSET over person', async () => {
  const pageRows: Row[] = [{
    id: 'p1', display_name: 'Jane Doe', role: 'buyer', status: 'new', location: null,
    assigned_agent: null, primary_email: 'jane@example.com', primary_phone: null,
    last_interaction_label: null, open_task_count: 0, active_deal_count: 0, interest_count: 0,
  }]
  const execute = fakeExecute(pageRows, 87)
  const result = await getClientAdminPage({ search: 'jane', page: 2, pageSize: 50 }, execute as never)
  assert.equal(result.total, 87)
  assert.equal(result.page, 2)
  assert.equal(result.rows.length, 1)
  assert.equal(result.rows[0].displayName, 'Jane Doe')
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')))
  assert.ok(execute.calls.some((c) => c.params.includes(50)))
})

test('promote: pickPrimaryIdentity prefers phone, then email, else null', () => {
  const phone = pickPrimaryIdentity(
    [{ value: 'a@b.com', normalized: 'a@b.com' }],
    [{ value: '+17875550134', normalized: '7875550134' }],
  )
  assert.equal(phone.kind, 'phone')
  assert.equal(phone.value, '7875550134')
  const email = pickPrimaryIdentity([{ value: 'Jane@Example.com', normalized: 'jane@example.com' }], [])
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
  assert.equal(groups.length, 2)
  assert.equal(groups.find((g) => g.identity.kind === 'phone')!.evidenceIds.length, 2)
  assert.equal(groups.find((g) => g.identity.kind === 'email')!.evidenceIds.length, 1)
})

test('promote: displayNameForEvidence uses a real name, else the identity label', () => {
  const named = [{ id: 'e1', displayName: 'Bob', source: 'gmail_contacts', emails: [{ value: 'b@c.com', normalized: 'b@c.com' }], phones: [] }]
  const namedGroups = groupEvidenceForPromotion(named as any)
  assert.equal(displayNameForEvidence(named as any, namedGroups[0]), 'Bob')
  const unnamed = [{ id: 'e1', displayName: null, source: 'apple_messages', emails: [], phones: [{ value: '+17875550134', normalized: '7875550134' }] }]
  const unnamedGroups = groupEvidenceForPromotion(unnamed as any)
  assert.equal(displayNameForEvidence(unnamed as any, unnamedGroups[0]), '+17875550134')
})

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

test('enrich: identityMatchKey normalizes phone/email to one stable key', () => {
  assert.equal(identityMatchKey('phone', '+1 (787) 555-0134'), 'phone:+17875550134')
  assert.equal(identityMatchKey('phone', '7875550134'), 'phone:+17875550134')
  assert.equal(identityMatchKey('email', 'Jane@Example.com'), 'email:jane@example.com')
  assert.equal(identityMatchKey('phone', ''), null)
})

test('enrich: buildContactIndex matches the same normalized identity and resolves the human-named contact', () => {
  const contacts = [
    { displayName: null, organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '+17875550134' },
    { displayName: 'Jane Doe', organization: 'Acme', displayAddress: '1 Calle Sol', identityType: 'phone', normalizedValue: '7875550134' },
  ]
  const index = buildContactIndex(contacts as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:+17875550134'], index)
  assert.equal(ambiguous, false)
  assert.equal(contact?.displayName, 'Jane Doe')
  assert.equal(contact?.organization, 'Acme')
  assert.equal(contact?.displayAddress, '1 Calle Sol')
})

test('enrich: resolveContactForIdentityKeys is ambiguous (never guesses) when names conflict', () => {
  const contacts = [
    { displayName: 'Jane Doe', organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '7875550134' },
    { displayName: 'Jane Smith', organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '+17875550134' },
  ]
  const index = buildContactIndex(contacts as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:+17875550134'], index)
  assert.equal(ambiguous, true)
  assert.equal(contact, null)
})

test('enrich: resolveContactForIdentityKeys returns null when no identity matches', () => {
  const index = buildContactIndex([
    { displayName: 'Jane', organization: null, displayAddress: null, identityType: 'email', normalizedValue: 'jane@example.com' },
  ] as any)
  const { contact, ambiguous } = resolveContactForIdentityKeys(['phone:+19999999999'], index)
  assert.equal(contact, null)
  assert.equal(ambiguous, false)
})

test('clients: getClientsPage marks nameResolved from the read-model sort priority', async () => {
  const pageRows: Row[] = [
    { person_id: 'p1', display_name: 'Jane Doe', name_sort_priority: 1, role: 'buyer', status: 'new', location: null, primary_email: null, primary_phone: null, assigned_agent: null, last_contact_label: null, sources: [] },
    { person_id: 'p2', display_name: '2039805771', name_sort_priority: 0, role: 'buyer', status: 'new', location: null, primary_email: null, primary_phone: '2039805771', assigned_agent: null, last_contact_label: null, sources: [] },
  ]
  const result = await getClientsPage({ page: 1, pageSize: 50 }, fakeExecute(pageRows, 2) as never)
  assert.equal(result.rows[0].nameResolved, true)
  assert.equal(result.rows[1].nameResolved, false)
})

test('clients: getClientContactHistory pages over the read model newest-first', async () => {
  const pageRows: Row[] = [{ interaction_id: 'i1', channel: 'imessage', direction: 'outbound', occurred_at: '2026-08-25T09:00:00.000Z', title: 'Re: showing', summary: 'Confirm 2pm' }]
  const execute = fakeExecute(pageRows, 45)
  const result = await getClientContactHistory('person-1', { page: 2, pageSize: 20 }, execute as never)
  assert.equal(result.total, 45)
  assert.equal(result.page, 2)
  assert.equal(result.pageSize, 20)
  assert.equal(result.rows.length, 1)
  assert.equal(result.rows[0].channel, 'imessage')
  assert.equal(result.rows[0].direction, 'outbound')
  assert.equal(result.rows[0].preview, 'Re: showing')
  assert.equal(result.rows[0].startedAt, '2026-08-25T09:00:00.000Z')
})

test('clients: getClientContactHistory is newest-first SQL against the read model (source guard)', () => {
  const src = readFileSync(new URL('../../db/contact-history.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_contact_history mv/.test(src))
  assert.ok(/order by mv\.occurred_at desc/.test(src))
  assert.ok(/limit \$\{pageSize\} offset \$\{offset\}/.test(src))
  assert.ok(/count\(\*\)::int as total/.test(src))
})

test('clients: name sort uses the read-model resolved-first priority (source guard)', () => {
  const src = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/mv\.name_sort_priority desc/.test(src))
  assert.ok(/mv\.display_name asc, mv\.person_id asc/.test(src))
  const migration = readFileSync(new URL('../../db/migrations/080_mv_client_read_models.sql', import.meta.url), 'utf8')
  assert.ok(/name_sort_priority/.test(migration))
})

test('clients: the deleted small Last contact pane is absent from the working pane', () => {
  const src = readFileSync(new URL('../../components/portal/client-manager.tsx', import.meta.url), 'utf8')
  assert.ok(!/heading="Last contact"/.test(src))
})

test('clients: Contact History is navy, fills its row, scrolls internally, server-paged (source guard)', () => {
  const src = readFileSync(new URL('../../components/portal/contact-history.tsx', import.meta.url), 'utf8')
  assert.ok(/variant="feature"/.test(src))
  assert.ok(/overflow-auto/.test(src))
  assert.ok(/flex-1 overflow-auto/.test(src))
  assert.ok(/pageSize/.test(src))
})

test('clients: CORE Clients working pane is the balanced relationship workspace (source guard)', () => {
  const src = readFileSync(new URL('../../components/portal/client-manager.tsx', import.meta.url), 'utf8')
  assert.ok(/CommandStatusBand/.test(src))
  assert.ok(/ratio="balanced"/.test(src))
  assert.ok(/lg:grid-cols-2 lg:gap-4/.test(src))
  assert.ok(/ClientCard/.test(src))
  assert.ok(/lg:grid-cols-2/.test(src))
  assert.ok(!/heading="Act"/.test(src))
  assert.ok(!/heading="Timeline"/.test(src))
  assert.ok(!/heading="Log a note"/.test(src))
  assert.ok(/ClientNotes/.test(src))
})

test('clients: directory/history reads come from materialized views, not L/ODS (source guard)', () => {
  const clients = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_directory mv/.test(clients))
  assert.ok(!/l_person|integration_relationship_evidence/.test(clients))
  const history = readFileSync(new URL('../../db/contact-history.ts', import.meta.url), 'utf8')
  assert.ok(/from mv_client_contact_history mv/.test(history))
  assert.ok(/distinct source_system as source/.test(history))
})

test('clients: refresh seam uses CONCURRENTLY (replay/refresh safe, unique index present)', () => {
  const src = readFileSync(new URL('../../db/client-read-models.ts', import.meta.url), 'utf8')
  assert.ok(/refresh materialized view concurrently/i.test(src))
  const migration = readFileSync(new URL('../../db/migrations/080_mv_client_read_models.sql', import.meta.url), 'utf8')
  assert.ok(/create unique index mv_client_directory_pk/.test(migration))
})