import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { getClientsPage } from '../../db/clients'
import { isHumanName } from '../../lib/relationship-intel/names'

// ---------------------------------------------------------------------------
// CLIENTS — Client directory name/search + unresolved-identity-sort correction
// (migration 095, read-model only). Proves display_name/location are searchable,
// identity search is preserved, and identity_fallback / :ABPerson display values
// sort as unresolved instead of resolved human names. No canonical Person change.
// ---------------------------------------------------------------------------

const MIG = readFileSync('db/migrations/095_mv_client_directory_name_search.sql', 'utf8')

type Row = Record<string, any>

function fakeExecute(pageRows: Row[], total: number) {
  const calls: Array<{ text: string; params: any[] }> = []
  const fn: any = (strings: TemplateStringsArray, ...params: any[]) => {
    const text = strings.join('__')
    calls.push({ text, params })
    if (text.includes('as total')) return [{ total }]
    if (text.includes('integration_relationship_evidence')) return []
    return pageRows
  }
  ;(fn as any).calls = calls
  return fn
}

test('dir-search 1: canonical display_name is included in Client directory search_text', () => {
  assert.ok(MIG.includes('p.display_name'), 'display_name feeds search_text')
  assert.ok(MIG.includes("concat_ws(' ', p.display_name, p.location, identity.search_text)"),
    'search_text = display_name + location + identities')
})

test('dir-search 2: email/phone identity search still works (search_text keeps identities)', () => {
  assert.ok(MIG.includes('string_agg(identity_value, \' \')'), 'identities still feed search_text')
  assert.ok(MIG.includes('identity.search_text'), 'identity search_text preserved in the concat')
})

test('dir-search 3: location is included in search_text', () => {
  assert.ok(MIG.includes('p.location'), 'location feeds search_text')
})

test('dir-search 4: identity_fallback Persons receive unresolved/low sort priority', () => {
  assert.ok(MIG.includes("b.display_name_source in ('unresolved', 'identity_fallback')"),
    'identity_fallback demoted alongside unresolved')
})

test('dir-search 5: :ABPerson display values do NOT sort as resolved human names', () => {
  assert.ok(MIG.includes("b.display_name ~ ':[A-Za-z0-9]+$'"), 'structured :Suffix display demoted')
  assert.equal(isHumanName('02A4C0A2-92F4-4C42-BA8C-D5467EDA4AE3:ABPerson'), false, ':ABPerson is not a human name')
  assert.equal(isHumanName('002D334C-B607-4DFE-A4E6-03429DFF7821:ABPerson'), false)
})

test('dir-search 6: legitimate human display names keep resolved/high priority', () => {
  assert.equal(isHumanName('Ami'), true)
  assert.equal(isHumanName('Alicia Geigel'), true)
  assert.ok(!MIG.includes("'source_evidence'"), 'source_evidence names are NOT demoted')
  assert.ok(!MIG.includes("'apple_contacts'"), 'apple_contacts names are NOT demoted')
})

test('dir-search 7: Ami is searchable by name (getClientsPage searches mv.search_text ilike)', async () => {
  const pageRows: Row[] = [
    { person_id: '99789478-be20-447e-8406-1ea9d7b61e12', display_name: 'Ami', name_sort_priority: 1, role: 'buyer', status: 'new', location: null, primary_email: null, primary_phone: '8609895020', assigned_agent: null, last_contact_label: 'Aug 28, 2026', sources: ['apple_messages'] },
  ]
  const execute = fakeExecute(pageRows, 1)
  const res = await getClientsPage({ search: 'Ami', page: 1, pageSize: 50 }, execute)
  assert.equal(res.total, 1)
  assert.equal(res.rows[0]?.displayName, 'Ami', 'Ami returned')
  assert.equal(res.rows[0]?.nameResolved, true, 'human name stays resolved')

  // The search token reaches the read-model guard as a bind value (nested inside
  // the raw guard fragment, which the fake executor does not flatten).
  const params = (execute as any).calls.flatMap((c: any) => c.params ?? [])
  const leaves: any[] = []
  const walk = (v: any) => {
    if (v && typeof v === 'object') {
      for (const k of Object.keys(v)) walk(v[k])
    } else {
      leaves.push(v)
    }
  }
  params.forEach(walk)
  assert.ok(leaves.includes('%Ami%'), 'the "Ami" token reaches the read-model search guard')
})

test('dir-search 8: migration 095 is read-model only (no canonical Person mutation)', () => {
  assert.ok(!/update\s+person/i.test(MIG), 'no update person')
  assert.ok(!/insert\s+into\s+person/i.test(MIG), 'no insert into person')
  assert.ok(!/delete\s+from\s+person/i.test(MIG), 'no delete from person')
  assert.ok(MIG.includes('create materialized view mv_client_directory'), 'read-model recreate only')
})
