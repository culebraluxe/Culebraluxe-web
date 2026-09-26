#!/usr/bin/env node
// ---------------------------------------------------------------------------
// CAPTURE REAL API ANSWERS AS UI CONTRACT FIXTURES.
//
//   pnpm ui:fixtures [base-url]        # default http://127.0.0.1:3000 (the dev server, with PORTAL_AUTH_BYPASS=1)
//
// Each screen's endpoint is asked once and its answer written to rust/ui/fixtures/<name>.json. The Rust tests decode
// those files into the endpoint's Response type (rust/ui/src/app/api.rs), so a payload shape that drifts from what the
// UI expects fails `cargo test` instead of a screen. The same files are the UI's fake data when a service is down.
//
// WHY REAL ANSWERS: a hand-written fixture repeats whatever the author assumed — on 2026-09-26 one encoded the DB Test
// payload wrapped in a `portal` field it does not have, and the test passed while the screen would have failed.
// ---------------------------------------------------------------------------
import { writeFileSync } from 'node:fs'

const base = (process.argv[2] ?? 'http://127.0.0.1:3000').replace(/\/+$/, '')
const out = new URL('../rust/ui/fixtures/', import.meta.url)

// name -> path. Add a line when a screen moves onto the Screen trait.
const ENDPOINTS = {
  'portal-page-db-test': '/api/portal/rust-ui/page?screen=db-test&',
  'clients-list': '/api/portal/rust-ui/clients?screen=clients&page=0&search=',
  'guest-session': '/api/rust-ui/guest',
}

// NO REAL PEOPLE IN GIT. The answers are real, so they carry client names, emails and phone numbers; what the tests
// need is the SHAPE. Every answer is reduced before it is written: arrays to their first two items, and any string that
// could be personal replaced with a placeholder of the same kind. Ids, dates, numbers, booleans and short lower-case
// codes (statuses, roles, channels) are kept, because decoding depends on them and they identify no one.
const KEEP = [
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i, // ids
  /^\d{4}-\d{2}-\d{2}([T ][\d:.]+(Z|[+-]\d{2}:?\d{2})?)?$/, // dates and times
  /^[a-z0-9_.-]{1,32}$/, // codes
]
function scrub(value) {
  if (Array.isArray(value)) return value.slice(0, 2).map(scrub)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, scrub(v)]))
  if (typeof value !== 'string' || value === '' || KEEP.some((re) => re.test(value))) return value
  if (value.includes('@')) return 'person@example.com'
  if (/^\+?[\d\s().-]{7,}$/.test(value)) return '+10000000000'
  if (/^https?:\/\//.test(value)) return 'https://example.com/'
  return 'Sample text'
}

let failed = 0
for (const [name, path] of Object.entries(ENDPOINTS)) {
  const response = await fetch(base + path)
  const text = await response.text()
  let json
  try {
    json = JSON.parse(text)
  } catch {
    json = null
  }
  if (!response.ok || json === null) {
    failed += 1
    console.error(`FAIL ${name}: ${response.status} ${text.slice(0, 120)}`)
    continue
  }
  writeFileSync(new URL(`${name}.json`, out), JSON.stringify(scrub(json), null, 2) + '\n')
  console.log(`ok   ${name} (${text.length} bytes received, scrubbed)`)
}
process.exit(failed ? 1 : 0)
