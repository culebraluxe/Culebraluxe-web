import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { getClientsPage } from '../../db/clients'
import { getClientAdminPage } from '../../db/client-admin'
import {
  pickPrimaryIdentity,
  groupEvidenceForPromotion,
  displayNameForEvidence,
} from '../../db/promote-evidence'
import { isHumanName } from '../../lib/relationship-intel/names'
import {
  identityMatchKey,
  buildContactIndex,
  findContactForIdentityKeys,
} from '../../db/enrich-people'

// ---------------------------------------------------------------------------
// CLIENTS — server-side pagination over the canonical `person` parent (no
// full-table materialization) + the evidence -> canonical Person promotion
// helpers. DB-free where possible (fake QueryExecutor).
// ---------------------------------------------------------------------------

type Row = Record<string, any>

/** Fake tagged-template QueryExecutor that answers COUNT + page queries. */
function fakeExecute(pageRows: Row[], total: number) {
  const calls: Array<{ text: string; params: any[] }> = []
  const fn: any = (strings: TemplateStringsArray, ...params: any[]) => {
    const text = strings.join('__')
    calls.push({ text, params })
    if (text.includes('as total from person')) return [{ total }]
    return pageRows
  }
  ;(fn as any).calls = calls
  return fn
}

test('clients: getClientsPage issues COUNT + LIMIT/OFFSET page over person', async () => {
  const pageRows: Row[] = [
    {
      id: 'p1',
      display_name: 'Jane Doe',
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
  const execute = fakeExecute(pageRows, 137)
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

  // separate COUNT(*) and a page query both ran
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')), 'COUNT query present')
  // SQL LIMIT/OFFSET flow as top-level params (50/page; page 3 -> offset 100)
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'pageSize 50 reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.params.includes(100)), 'offset 100 on page 3')
})

test('clients: getClientsPage search/filter/sort/limit are SQL (source guard)', () => {
  const src = readFileSync(new URL('../../db/clients.ts', import.meta.url), 'utf8')
  assert.ok(/ilike \$\{like\}/.test(src), 'search is a SQL ILIKE predicate')
  assert.ok(/limit \$\{pageSize\} offset \$\{offset\}/.test(src), 'SQL LIMIT/OFFSET')
  assert.ok(/count\(\*\)::int as total/.test(src), 'separate COUNT')
  assert.ok(/and \(\$\{status\}::text is null or p.status = \$\{status\}\)/.test(src), 'status filter in SQL')
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

test('enrich: buildContactIndex matches the same normalized identity and prefers a human-named contact', () => {
  const contacts = [
    { displayName: null, organization: null, displayAddress: null, identityType: 'phone', normalizedValue: '+17875550134' },
    { displayName: 'Jane Doe', organization: 'Acme', displayAddress: '1 Calle Sol', identityType: 'phone', normalizedValue: '7875550134' },
  ]
  const index = buildContactIndex(contacts as any)
  const contact = findContactForIdentityKeys(['phone:7875550134'], index)
  assert.equal(contact?.displayName, 'Jane Doe')
  assert.equal(contact?.organization, 'Acme')
  assert.equal(contact?.displayAddress, '1 Calle Sol')
})

test('enrich: findContactForIdentityKeys returns null when no identity matches', () => {
  const index = buildContactIndex([
    { displayName: 'Jane', organization: null, displayAddress: null, identityType: 'email', normalizedValue: 'jane@example.com' },
  ] as any)
  assert.equal(findContactForIdentityKeys(['phone:9999999999'], index), null)
})

