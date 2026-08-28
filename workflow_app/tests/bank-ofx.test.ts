import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  detectFormat,
  formatMoney,
  maskAccount,
  ofxDateOnly,
  parseBankStatement,
  parseOfxDate,
  sourceAccountIdentity,
} from '../../lib/bank-ofx'

// ---------------------------------------------------------------------------
// BANK-OFX — narrow QBO/OFX parser (pure, zero DB).
// Legacy SGML and XML both normalize to the same source-neutral model.
// ---------------------------------------------------------------------------

const FIXTURE = join(
  __dirname,
  '..',
  '..',
  'smoke-fixtures',
  'accounting',
  'culebraluxe_fake_bank_variety_2026-07.qbo',
)

test('parses the legacy SGML QBO fixture into 27 valid transactions', () => {
  const content = readFileSync(FIXTURE, 'utf8')
  const parsed = parseBankStatement(content, 'culebraluxe_fake_bank_variety_2026-07.qbo')
  const s = parsed.statement
  assert.equal(s.sourceFormat, 'QBO')
  assert.equal(s.syntaxKind, 'sgml')
  assert.equal(parsed.parsedCount, 27)
  assert.equal(s.transactions.length, 27)
  assert.deepEqual(parsed.rejected, [])
  assert.equal(s.bankId, '999999999')
  assert.equal(s.accountId, '0000123456789')
  assert.equal(s.accountType, 'CHECKING')
  assert.equal(s.currencyCode, 'USD')
  assert.equal(s.statementStart, '2026-07-01')
  assert.equal(s.statementEnd, '2026-07-31')
})

test('legacy QBO: FITID, signed amount, NAME, MEMO, CHECKNUM, REFNUM preserved', () => {
  const content = readFileSync(FIXTURE, 'utf8')
  const { statement } = parseBankStatement(content, 'f.qbo')
  const first = statement.transactions.find((t) => t.fitid === 'FAKE-20260701-0001')!
  assert.equal(first.transactionType, 'DEP')
  assert.equal(first.amount, 8450) // credit preserved positive
  assert.equal(first.payeeName, 'CULEBRALUXE COMMISSION DEPOSIT')
  assert.equal(first.memo, 'Closing commission - Villa Esperanza')
  assert.equal(first.referenceNumber, 'DEP-070126-A')

  const check = statement.transactions.find((t) => t.fitid === 'FAKE-20260705-0004')!
  assert.equal(check.transactionType, 'CHECK')
  assert.equal(check.amount, -625) // debit preserved negative
  assert.equal(check.checkNumber, '1042')

  const both = statement.transactions.find((t) => t.fitid === 'FAKE-20260715-0016')!
  assert.equal(both.checkNumber, '1043')
  assert.equal(both.referenceNumber, 'CHK-1043')
})

test('XML OFX normalizes to the same internal model', () => {
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<OFX>
  <SIGNONMSGSRSV1><SONRS><FI><ORG>FAKE</ORG><FID>8888</FID></FI></SONRS></SIGNONMSGSRSV1>
  <BANKMSGSRSV1><STMTTRNRS><STMTRS>
    <CURDEF>USD</CURDEF>
    <BANKACCTFROM><BANKID>7777</BANKID><ACCTID>1234567890</ACCTID><ACCTTYPE>CHECKING</ACCTTYPE></BANKACCTFROM>
    <BANKTRANLIST><DTSTART>20260701</DTSTART><DTEND>20260731</DTEND>
      <STMTTRN>
        <TRNTYPE>DEP</TRNTYPE>
        <DTPOSTED>20260702120000[0:GMT]</DTPOSTED>
        <TRNAMT>250.50</TRNAMT>
        <FITID>XML-0001</FITID>
        <NAME>XML PAYEE</NAME>
        <MEMO>XML memo</MEMO>
      </STMTTRN>
    </BANKTRANLIST>
  </STMTRS></STMTTRNRS></BANKMSGSRSV1>
</OFX>`
  const { statement, parsedCount } = parseBankStatement(xml, 'statement.ofx')
  assert.equal(statement.sourceFormat, 'OFX')
  assert.equal(statement.syntaxKind, 'xml')
  assert.equal(parsedCount, 1)
  assert.equal(statement.bankId, '7777')
  assert.equal(statement.accountId, '1234567890')
  assert.equal(statement.currencyCode, 'USD')
  const tx = statement.transactions[0]
  assert.equal(tx.fitid, 'XML-0001')
  assert.equal(tx.amount, 250.5)
  assert.equal(tx.payeeName, 'XML PAYEE')
  assert.equal(tx.postedAt, '2026-07-02T12:00:00.000Z')
})

test('missing FITID transaction is rejected and counted, not fabricated', () => {
  const content = `OFXHEADER:100
DATA:OFXSGML
<OFX>
<STMTTRN><TRNTYPE>POS</TRNTYPE><DTPOSTED>20260701103000</DTPOSTED><TRNAMT>-10.00</TRNAMT><NAME>NO FITID</NAME></STMTTRN>
</OFX>`
  const parsed = parseBankStatement(content, 'f.qbo')
  assert.equal(parsed.parsedCount, 1)
  assert.equal(parsed.statement.transactions.length, 0)
  assert.equal(parsed.rejected.length, 1)
  assert.equal(parsed.rejected[0].reason, 'missing FITID')
})

test('invalid amount and unusable date are bounded transaction rejections', () => {
  const content = `<OFX>
<STMTTRN><FITID>BAD-AMT</FITID><DTPOSTED>20260701</DTPOSTED><TRNAMT>not-a-number</TRNAMT></STMTTRN>
<STMTTRN><FITID>BAD-DATE</FITID><DTPOSTED>NOTADATE</DTPOSTED><TRNAMT>-5.00</TRNAMT></STMTTRN>
<STMTTRN><FITID>OK</FITID><DTPOSTED>20260701</DTPOSTED><TRNAMT>1.00</TRNAMT></STMTTRN>
</OFX>`
  const parsed = parseBankStatement(content, 'f.ofx')
  assert.equal(parsed.statement.transactions.length, 1)
  assert.equal(parsed.statement.transactions[0].fitid, 'OK')
  assert.equal(parsed.rejected.length, 2)
  assert.ok(parsed.rejected.some((r) => r.reason.includes('invalid amount')))
  assert.ok(parsed.rejected.some((r) => r.reason.includes('unusable posted date')))
})

test('raw STMTTRN fragment is preserved for lineage', () => {
  const content = readFileSync(FIXTURE, 'utf8')
  const { statement } = parseBankStatement(content, 'f.qbo')
  const tx = statement.transactions[0]
  assert.ok(tx.rawSourceFragment.includes('<STMTTRN>'))
  assert.ok(tx.rawSourceFragment.includes('<FITID>FAKE-20260701-0001'))
})

test('format detection: rejects non-OFX-family files', () => {
  assert.throws(() => detectFormat('hello, this is a CSV', 'bank.csv'), /Not an OFX-family/)
  assert.throws(() => detectFormat('not a bank file at all', 'anything.txt'), /Not an OFX-family/)
})

test('maskAccount masks to the last four digits', () => {
  assert.equal(maskAccount('0000123456789'), '****6789')
  assert.equal(maskAccount(null), null)
  assert.equal(maskAccount(undefined), null)
})

test('ofxDateOnly extracts the calendar date from a raw OFX value', () => {
  assert.equal(ofxDateOnly('20260701235959[-4:AST]'), '2026-07-01')
  assert.equal(ofxDateOnly('20260701'), '2026-07-01')
  assert.equal(ofxDateOnly(undefined), null)
})

test('sourceAccountIdentity is deterministic and distinguishes accounts', () => {
  assert.equal(sourceAccountIdentity('0000123456789'), '0000123456789')
  assert.equal(sourceAccountIdentity('1234567890'), '1234567890')
  assert.equal(sourceAccountIdentity(null), 'UNKNOWN_ACCOUNT')
})

test('formatMoney renders signed currency', () => {
  assert.equal(formatMoney(-12431.28), '-12,431.28')
  assert.equal(formatMoney(8900), '8,900.00')
})

// ---------------------------------------------------------------------------
// OFX date normalization across common forms
// ---------------------------------------------------------------------------

test('parseOfxDate supports the common OFX date forms', () => {
  // YYYYMMDD
  assert.equal(parseOfxDate('20260701'), '2026-07-01T00:00:00.000Z')
  // YYYYMMDDHHMMSS (no timezone -> treated as UTC)
  assert.equal(parseOfxDate('20260701103000'), '2026-07-01T10:30:00.000Z')
  // timezone offset [-4:AST]
  assert.equal(parseOfxDate('20260701103000[-4:AST]'), '2026-07-01T14:30:00.000Z')
  // [0:GMT]
  assert.equal(parseOfxDate('20260702120000[0:GMT]'), '2026-07-02T12:00:00.000Z')
  // fractional seconds
  assert.equal(parseOfxDate('20260731145959.125[-4:AST]'), '2026-07-31T18:59:59.125Z')
  // trailing Z
  assert.equal(parseOfxDate('20260701103000Z'), '2026-07-01T10:30:00.000Z')
  // unusable
  assert.equal(parseOfxDate('NOTADATE'), null)
})

