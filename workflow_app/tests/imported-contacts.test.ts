import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { getImportedContacts } from '../../db/imported-contacts'
import { projectLPersonFromStaged } from '../../lib/intake/l-person-projection'

// ---------------------------------------------------------------------------
// SUPPORT-2 — Clients imported view + canonical-preservation proofs.
// Test 10 uses a fake QueryExecutor (DB-free); tests 11-12 are source/shape
// guards.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

/** Minimal fake tagged-template QueryExecutor that answers count + page queries. */
function fakeExecute(pageRows: Row[], total: number) {
  const calls: Array<{ text: string; params: any[] }> = []
  const fn: any = (strings: TemplateStringsArray, ...params: any[]) => {
    const text = strings.reduce(
      (acc, s, i) => acc + s + (i < params.length ? `$${i + 1}` : ''),
      '',
    )
    calls.push({ text: text.replace(/\s+/g, ' ').trim().toLowerCase(), params })
    const isCount = /select count\(\*\)/.test(text.toLowerCase())
    return Promise.resolve(isCount ? [{ total }] : pageRows)
  }
  fn.calls = calls
  return fn
}

test('SUPPORT-2 test 10: Clients imported view returns / searches / paginates load records', async () => {
  const pageRows: Row[] = [
    {
      id: 'lp-1',
      source: 'apple_contacts',
      source_account: 'culebraluxe-lisa-icloud-contacts',
      source_contact_id: 'CONTACT-1:ABPerson',
      source_revision: 1,
      display_name: 'Jane Doe',
      organization: 'Culebra Construction',
      display_address: '1 Calle Sol, Culebra, PR 00775',
      email: 'jane@example.com',
      phone: '+17875550134',
      reconciliation_status: 'unreviewed',
      projected_at: '2026-08-24T00:00:00.000Z',
    },
  ]
  const execute = fakeExecute(pageRows, 25)

  const result = await getImportedContacts(
    { search: 'jane', page: 2, pageSize: 10 },
    execute as never,
  )

  assert.equal(result.total, 25)
  assert.equal(result.page, 2)
  assert.equal(result.pageSize, 10)
  assert.equal(result.rows.length, 1)
  const row = result.rows[0]
  assert.equal(row.displayName, 'Jane Doe')
  assert.equal(row.organization, 'Culebra Construction')
  assert.equal(row.email, 'jane@example.com')
  assert.equal(row.phone, '+17875550134')
  assert.equal(row.displayAddress, '1 Calle Sol, Culebra, PR 00775')
  assert.equal(row.reconciliationStatus, 'unreviewed')
  assert.equal(row.source, 'apple_contacts')

  // The search value flows through as a parameterized ILIKE pattern.
  assert.ok(
    execute.calls.some((c) => c.params.includes('%jane%')),
    'search must reach the query as a parameter',
  )
  // The page size flows through as a LIMIT param.
  assert.ok(execute.calls.some((c) => c.params.includes(10)), 'pageSize must reach the query')
})

test('SUPPORT-2 test 11: canonical Clients view is preserved on the page', () => {
  const page = readFileSync(
    new URL('../../app/portal/clients/page.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(page.includes('ClientManager'), 'canonical ClientManager is the CORE Clients UX')
  assert.ok(!page.includes('ClientAdmin'), 'Client Administration is not on CORE Clients (moved to OPPS)')
  assert.ok(!/ClientsTabBar/.test(page), 'staging strip removed from CORE Clients')
  assert.ok(!/ImportedContactsPanel/.test(page), 'imported pane removed from CORE Clients')
  assert.ok(page.includes('ClientManager />'), 'paged ClientManager is self-contained')
  assert.ok(!/app\/portal\/clients\/imported/.test(page), 'no second top-level Clients page')
})

test('SUPPORT-2 test 14: Client Administration is preserved on an OPPS route', () => {
  const page = readFileSync(
    new URL('../../app/portal/client-admin/page.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(page.includes('ClientAdmin'), 'ClientAdmin still rendered under OPPS')
  assert.ok(
    page.includes('export const dynamic'),
    'OPPS Client Administration route is request-time dynamic',
  )
})

test('SUPPORT-2 test 13: Imported Contacts stewardship is preserved on an OPPS surface', () => {
  const page = readFileSync(
    new URL('../../app/portal/identity-quality/page.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(page.includes('ImportedContactsPanel'), 'Imported Contacts preserved (OPPS stewardship)')
  assert.ok(page.includes('getImportedContactsCount'), 'imported count still fetched')
})

test('SUPPORT-2 test 12: no private contact payload appears in the load projection / snapshots', () => {
  // The load projection emits only flattened l_person fields + child identity/
  // address rows — never the raw `profile`/`emails`/`phones`/`postalAddresses`
  // arrays that carry the full private payload.
  const out = projectLPersonFromStaged({
    stagedProfileId: 'staged-1',
    intakeBatchId: 'batch-1',
    source: 'apple_contacts',
    sourceAccount: 'culebraluxe-lisa-icloud-contacts',
    sourceContactId: 'CONTACT-1:ABPerson',
    revision: 1,
    payloadFingerprint: 'fp',
    reconciliationStatus: 'unreviewed',
    candidatePersonId: null,
    profile: {
      name: { given: 'Jane', family: 'Doe' },
      organization: 'Acme',
      department: '',
      jobTitle: '',
      emails: [{ label: '_$!<Work>!$_', value: 'jane@example.com' }],
      phones: [{ label: '_$!<Mobile>!$_', value: '+1 787 555 0100' }],
      postalAddresses: [
        { label: '_$!<Home>!$_', street: '1 Calle Sol', city: 'Culebra', state: 'PR', postalCode: '00775', country: 'Puerto Rico', isoCountryCode: 'PR' },
      ],
    },
  })
  const json = JSON.stringify(out)
  assert.ok(!json.includes('postalAddresses'), 'raw postalAddresses array must not leak')
  assert.ok(!json.includes('emails'), 'raw emails array must not leak')
  assert.ok(!json.includes('phones'), 'raw phones array must not leak')
  // Identity values are preserved deliberately (they are the load identities),
  // but the full source profile object is not embedded.
  assert.ok(json.includes('jane@example.com'), 'email identity is preserved in l_person_identity')
})
