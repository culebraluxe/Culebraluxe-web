import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  parseGmailCensus,
  parseGmailCensusRow,
  parseCsvLine,
} from '../../lib/relationship-intel/gmail-census'

// ---------------------------------------------------------------------------
// REL-INTEL — bounded Gmail census import: row parsing, replay, balancing,
// formula protection, quarantine. No database.
// ---------------------------------------------------------------------------

const HEADER =
  'normalized_email,display_name_candidates,source_system,source_account_token,first_seen_at,last_seen_at,first_inbound_at,last_inbound_at,first_outbound_at,last_outbound_at,inbound_message_count,outbound_message_count,cc_or_bcc_message_count,distinct_thread_count,last_direction,last_gmail_message_id,last_gmail_thread_id,last_content_reference,two_way_relationship_evidence,lisa_initiated_evidence,multi_recipient_or_bulk_evidence,automated_or_service_evidence,organization_domain_evidence,airbnb_or_property_operations_evidence,canonical_person_id,apple_l_person_id,reconciliation_outcome,acquisition_error_or_limitation'

function row(email: string, extra = ''): string {
  return `${email},Jane Doe,gmail,opaque-account,2012-01-01,2013-12-31,2012-01-01,2013-12-31,2012-02-01,2013-11-01,12,4,2,6,outbound,,,,true,true,false,false,false,false,,,unmatched_relationship_candidate,partial sweep omits early history${extra}`
}

test('REL-INTEL: a valid correspondent row maps to neutral evidence', () => {
  const r = parseGmailCensusRow(row('Jane.Doe@Example.com'), 2)
  assert.ok(r.ok)
  const e = r.row.evidence
  assert.equal(e.source, 'gmail_contacts')
  assert.equal(e.sourceIdentityKey, 'jane.doe@example.com')
  assert.equal(e.displayName, 'Jane Doe')
  assert.equal(e.hasEmail, true)
  assert.equal(e.hasPhone, false)
  assert.equal(e.isTwoWay, true)
  assert.equal(e.isOwnerInitiated, true)
  assert.equal(e.inboundCount, 12)
  assert.equal(e.outboundCount, 4)
  assert.ok(e.coverageNote?.includes('partial'))
})

test('REL-INTEL: a replayed row yields the same fingerprint (idempotency)', () => {
  const a = parseGmailCensusRow(row('Jane.Doe@Example.com'), 2)
  const b = parseGmailCensusRow(row('Jane.Doe@Example.com'), 3)
  assert.ok(a.ok && b.ok)
  assert.equal(a.row.fingerprint, b.row.fingerprint)
})

test('REL-INTEL: a changed row yields a different fingerprint', () => {
  const a = parseGmailCensusRow(row('Jane.Doe@Example.com'), 2)
  const b = parseGmailCensusRow(row('Jane.Doe@Example.com'), 2).ok
    ? (() => {
        const cols = row('Jane.Doe@Example.com').split(',')
        cols[10] = '99' // different inbound count
        return parseGmailCensusRow(cols.join(','), 2)
      })()
    : null
  assert.ok(a.ok && b && b.ok)
  assert.notEqual(a.row.fingerprint, b.row.fingerprint)
})

test('REL-INTEL: missing email row is rejected with a reason', () => {
  const r = parseGmailCensusRow(row('   '), 2)
  assert.equal(r.ok, false)
  assert.equal(r.reason, 'empty')
})

test('REL-INTEL: CSV formula-like display name is neutralized', () => {
  const raw = row('a@example.com', '')
  const cols = raw.split(',')
  cols[1] = '=HYPERLINK("http://evil")'
  const r = parseGmailCensusRow(cols.join(','), 2)
  assert.ok(r.ok)
  assert.ok(r.row.evidence.displayName?.startsWith("'="))
})

test('REL-INTEL: batch balances declared = accepted + rejected + quarantined', () => {
  const csv = [
    HEADER,
    row('a@example.com'),
    row('b@example.com'),
    '  ,Jane,apple,acc,,,,,,,,,,,,,,,,,,,,,missing_email_limitation', // missing email -> rejected
    row('c@example.com'),
  ].join('\n')
  const out = parseGmailCensus(csv)
  assert.equal(out.declared, 4)
  assert.equal(out.accepted + out.rejected + out.quarantined, out.declared)
  assert.equal(out.rejected, 1)
})

test('REL-INTEL: duplicate emails are deduplicated and reported separately', () => {
  const csv = [
    HEADER,
    row('dup@example.com'),
    row('dup@example.com'),
    row('other@example.com'),
  ].join('\n')
  const out = parseGmailCensus(csv)
  assert.equal(out.accepted, 2)
  assert.equal(out.deduplicated, 1)
  // Balance does NOT include dedup (reported separately).
  assert.equal(out.declared, 3)
})

test('REL-INTEL: quoted CSV fields are parsed correctly', () => {
  const cells = parseCsvLine('"Doe, Jane",jane@example.com,"a,b"')
  assert.equal(cells.length, 3)
  assert.equal(cells[0], 'Doe, Jane')
  assert.equal(cells[2], 'a,b')
})
