import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  fingerprint,
  normalizeEmail,
  normalizePhone,
  sanitizeSpreadsheetCell,
} from '../../lib/relationship-intel/normalize'

// ---------------------------------------------------------------------------
// REL-INTEL — deterministic email/phone normalization + replay fingerprint.
// No database. Focused proofs required by the REL-INTEL order (Part 13).
// ---------------------------------------------------------------------------

test('REL-INTEL: email casing and whitespace normalize to lowercase', () => {
  const r = normalizeEmail('  Jane.Doe@Example.COM  ')
  assert.ok(r.ok)
  assert.equal(r.value, 'jane.doe@example.com')
})

test('REL-INTEL: distinct plus-addresses are preserved, not collapsed', () => {
  const a = normalizeEmail('jane+home@example.com')
  const b = normalizeEmail('jane+work@example.com')
  assert.ok(a.ok && b.ok)
  assert.notEqual(a.value, b.value)
})

test('REL-INTEL: structurally invalid email is quarantined, not deleted', () => {
  const r = normalizeEmail('not-an-email')
  assert.equal(r.ok, false)
  assert.equal(r.reason, 'invalid_format')
  assert.equal(r.original, 'not-an-email')
})

test('REL-INTEL: empty email is quarantined', () => {
  const r = normalizeEmail('   ')
  assert.equal(r.ok, false)
  assert.equal(r.reason, 'empty')
})

test('REL-INTEL: US/Puerto Rico phone with leading country code normalizes', () => {
  const r = normalizePhone('+1 (787) 555-0134')
  assert.ok(r.ok)
  assert.equal(r.value, '7875550134')
})

test('REL-INTEL: 10-digit phone normalizes as-is', () => {
  const r = normalizePhone('7875550134')
  assert.ok(r.ok)
  assert.equal(r.value, '7875550134')
})

test('REL-INTEL: ambiguous international phone is quarantined, not guessed', () => {
  const r = normalizePhone('+44 20 7946 0958')
  // 12 digits without leading country-1 -> not reliably US/PR.
  assert.equal(r.ok, false)
  assert.equal(r.reason, 'ambiguous_international')
})

test('REL-INTEL: spreadsheet formula injection is neutralized', () => {
  assert.equal(sanitizeSpreadsheetCell('=cmd|/C calc!A0'), "'=cmd|/C calc!A0")
  assert.equal(sanitizeSpreadsheetCell('@sum'), "'@sum")
  assert.equal(sanitizeSpreadsheetCell('plain'), 'plain')
})

test('REL-INTEL: fingerprint is deterministic and sensitive to input', () => {
  const a = fingerprint('jane@example.com|2026-01-01')
  const b = fingerprint('jane@example.com|2026-01-01')
  const c = fingerprint('jane@example.com|2026-01-02')
  assert.equal(a, b)
  assert.notEqual(a, c)
})
