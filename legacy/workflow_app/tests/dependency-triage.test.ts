import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// ---------------------------------------------------------------------------
// ENG-FORGE-DEPENDENCY-AUDIT-01 — the dependency scan is scheduled, the full
// pnpm graph is read, and every advisory it reports is triaged once with a
// reason. The test reads the three artefacts the scan contract depends on:
// package.json (the script), .github/workflows/gates.yml (the schedule) and
// docs/agent/DEPENDENCY-TRIAGE.md (the triage ledger). It does not run the
// scanner — the scanner is the live instrument, this is its regression guard.
// ---------------------------------------------------------------------------

const ROOT = new URL('../../../', import.meta.url)
const read = (relativePath: string): string => readFileSync(new URL(relativePath, ROOT), 'utf8')

const LEDGER = 'docs/agent/DEPENDENCY-TRIAGE.md'
const GATES = '.github/workflows/gates.yml'
const PACKAGE_JSON = 'package.json'

const CLASSIFICATIONS = new Set(['reachable-in-production', 'dev-only', 'not-reachable'])
const ADVISORY = /^GHSA-[a-z0-9-]+$/i

type TriageRow = {
  package: string
  version: string
  advisory: string
  severity: string
  classification: string
  reason: string
}

// Frozen advisory set measured 2026-09-18 by
// `osv-scanner scan source -L pnpm-lock.yaml --all-packages` (45 rows, 43 unique
// advisory ids). A ledger that drops one of these has lost a triage decision.
const SCANNED = new Set<string>([
  'next@16.3.0|GHSA-2xp9-vwfh-vxw4',
  'next@16.3.0|GHSA-p293-qw3h-jr36',
  'brace-expansion@5.0.6|GHSA-3jxr-9vmj-r5cp',
  'brace-expansion@5.0.6|GHSA-mh99-v99m-4gvg',
  'brace-expansion@5.0.6|GHSA-rgw5-rvv9-x895',
  'browserslist@4.28.1|GHSA-73wf-gq98-2v4g',
  'browserslist@4.28.1|GHSA-c83g-rgw3-j3cx',
  'fast-uri@3.1.2|GHSA-4c8g-83qw-93j6',
  'fast-uri@3.1.2|GHSA-7p8r-x3mc-p8w7',
  'fast-uri@3.1.2|GHSA-f65p-4m7j-42xc',
  'fast-uri@3.1.2|GHSA-fph4-wmhf-6fwf',
  'fast-uri@3.1.2|GHSA-jqff-g426-hqxp',
  'fast-uri@3.1.2|GHSA-v2hh-gcrm-f6hx',
  'ip-address@10.2.0|GHSA-mwp4-54f8-5fhr',
  'js-yaml@4.2.0|GHSA-2883-xcg3-v3hh',
  'js-yaml@4.2.0|GHSA-52cp-r559-cp3m',
  'js-yaml@4.2.0|GHSA-5p4m-2wfm-xmqj',
  'nanoid@3.3.11|GHSA-28wg-ghj8-5hjv',
  'nanoid@3.3.11|GHSA-2v37-7h3g-55p8',
  'nanoid@3.3.11|GHSA-xwg4-73v4-xw9w',
  'nanoid@3.3.16|GHSA-2v37-7h3g-55p8',
  'postcss@8.5.6|GHSA-6g55-p6wh-862q',
  'postcss@8.5.6|GHSA-r28c-9q8g-f849',
  'sharp@0.35.3|GHSA-rgj7-g3m4-5g8c',
  '@hono/node-server@1.19.14|GHSA-frvp-7c67-39w9',
  'baseline-browser-mapping@2.9.19|GHSA-w5vr-8v7q-w6rv',
  'hono@4.12.25|GHSA-54fx-42gc-7vw4',
  'hono@4.12.25|GHSA-8j4g-w8fx-2239',
  'hono@4.12.25|GHSA-crvj-82cr-hjcx',
  'hono@4.12.25|GHSA-f23p-vx2j-j53r',
  'hono@4.12.25|GHSA-g6gw-c38x-mqfc',
  'hono@4.12.25|GHSA-gqvv-2mrq-wpjv',
  'hono@4.12.25|GHSA-hvrm-45r6-mjfj',
  'hono@4.12.25|GHSA-w62v-xxxg-mg59',
  'hono@4.12.25|GHSA-xgm2-5f3f-mvvc',
  'ip-address@10.2.0|GHSA-22jq-vg5j-6vgg',
  'ip-address@10.2.0|GHSA-4xrf-jv44-h6hh',
  'postcss@8.5.19|GHSA-fxqj-rqcc-2cmp',
  'postcss@8.5.6|GHSA-fxqj-rqcc-2cmp',
  'postcss@8.5.6|GHSA-qx2v-qp2m-jg93',
  'qs@6.15.2|GHSA-4mjr-xmp4-gh2g',
  'qs@6.15.2|GHSA-x5fp-wj9c-mxmx',
  'body-parser@2.2.2|GHSA-v422-hmwv-36x6',
  'hono@4.12.25|GHSA-79qm-7rj5-m7r9',
  'postcss-selector-parser@7.1.1|GHSA-w9m9-85wc-3x92',
])

/** Read the markdown triage table into rows. Header and separator rows are skipped. */
function parseTriageLedger(markdown: string): TriageRow[] {
  const rows: TriageRow[] = []
  for (const line of markdown.split('\n')) {
    if (!line.trim().startsWith('|')) continue
    const cells = line.split('|').map((cell) => cell.trim())
    const advisory = cells[3]
    if (!advisory || !ADVISORY.test(advisory)) continue
    rows.push({
      package: cells[1] ?? '',
      version: cells[2] ?? '',
      advisory,
      severity: cells[4] ?? '',
      classification: cells[5] ?? '',
      reason: cells[6] ?? '',
    })
  }
  return rows
}

/** Return the missing pieces of a triage row. Empty means fully triaged. */
function triageProblems(row: TriageRow): string[] {
  const problems: string[] = []
  if (!row.package) problems.push('package')
  if (!row.version) problems.push('version')
  if (!ADVISORY.test(row.advisory)) problems.push('advisory')
  if (!CLASSIFICATIONS.has(row.classification)) problems.push('classification')
  if (row.reason.trim().length < 10) problems.push('reason')
  return problems
}

test('scan:deps reads the full pnpm graph', () => {
  const pkg = JSON.parse(read(PACKAGE_JSON)) as { scripts: Record<string, string> }
  const script = pkg.scripts['scan:deps']
  assert.ok(script, 'package.json has no scan:deps script')
  assert.match(script, /osv-scanner/)
  assert.match(script, /pnpm-lock\.yaml/)
  assert.match(script, /--all-packages/)
})

test('scheduled dependency scan is wired', () => {
  const gates = read(GATES)
  assert.match(gates, /^on:/m)
  assert.match(gates, /schedule:/)
  assert.match(gates, /cron:/)
  assert.match(gates, /pnpm scan:deps/)
})

test('triage ledger lists every advisory by id and package', () => {
  const rows = parseTriageLedger(read(LEDGER))
  const keys = new Set(rows.map((row) => `${row.package}@${row.version}|${row.advisory}`))
  for (const key of SCANNED) {
    assert.ok(keys.has(key), `triage ledger is missing ${key}`)
  }
  assert.equal(keys.size, rows.length, 'triage ledger has a duplicate package/version/advisory row')
})

test('every advisory is triaged with a classification and reason', () => {
  const rows = parseTriageLedger(read(LEDGER))
  assert.ok(rows.length >= SCANNED.size, `ledger has ${rows.length} rows, expected at least ${SCANNED.size}`)
  for (const row of rows) {
    assert.deepEqual(
      triageProblems(row),
      [],
      `untriaged: ${row.package}@${row.version} ${row.advisory}`,
    )
  }
  const criticals = rows.filter((row) => row.severity.toUpperCase() === 'CRITICAL')
  assert.ok(criticals.length >= 2, 'the two CRITICAL next advisories are not in the ledger')
  for (const critical of criticals) {
    assert.equal(
      critical.classification,
      'reachable-in-production',
      `${critical.package}@${critical.version} ${critical.advisory} is CRITICAL but not reachable-in-production`,
    )
  }
})

test('an untriaged advisory fails the triage check', () => {
  const untriaged: TriageRow = {
    package: 'left-pad',
    version: '1.0.0',
    advisory: 'GHSA-0000-0000-0000',
    severity: 'HIGH',
    classification: '',
    reason: '',
  }
  assert.deepEqual(triageProblems(untriaged), ['classification', 'reason'])

  const invented: TriageRow = { ...untriaged, classification: 'probably-fine', reason: 'looks okay to me' }
  assert.deepEqual(triageProblems(invented), ['classification'])
})

test('a new critical advisory is visible without a manual command', () => {
  const gates = read(GATES)
  assert.match(gates, /schedule:/)
  assert.match(gates, /cron:/)
  assert.match(gates, /pnpm scan:deps/)
  const osvStep = gates
    .split(/\n\s*-\s+name:/)
    .find((step) => step.includes('osv-scanner') && step.includes('scan:deps'))
  assert.ok(osvStep, 'the gates workflow has no osv-scanner step invoking scan:deps')
  assert.doesNotMatch(osvStep, /continue-on-error/)

  const rows = parseTriageLedger(read(LEDGER))
  const nextCriticals = rows.filter(
    (row) => row.package === 'next' && row.severity.toUpperCase() === 'CRITICAL',
  )
  assert.equal(nextCriticals.length, 2, 'the two CRITICAL next advisories are not both in the ledger')
  for (const row of nextCriticals) assert.equal(row.classification, 'reachable-in-production')
})
