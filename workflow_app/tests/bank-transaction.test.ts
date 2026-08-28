import { test } from 'node:test'
import assert from 'node:assert/strict'

import { insertBankTransactions } from '../../db/bank-transaction'
import type { BankTransaction } from '../../lib/bank-ofx'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// BANK-OFX — replay-safe repository load (in-memory unique-constraint model).
// ---------------------------------------------------------------------------

function tx(overrides: Partial<BankTransaction> & { fitid: string }): BankTransaction {
  return {
    transactionType: 'POS',
    postedAt: '2026-07-01T12:00:00.000Z',
    userInitiatedAt: null,
    amount: -25,
    payeeName: 'PAYEE',
    memo: null,
    checkNumber: null,
    referenceNumber: null,
    rawSourceFragment: '<STMTTRN></STMTTRN>',
    ...overrides,
  }
}

type StoredRow = { id: string; reconciliation_status: string }

/** In-memory model of the l_bank_transaction unique(source_system, source_account, fitid). */
function makeStore() {
  const rows = new Map<string, StoredRow>()
  let n = 0
  const execute: QueryExecutor = ((strings: TemplateStringsArray, ...params: unknown[]) => {
    const sql = strings[0] ?? ''
    if (sql.includes('insert into l_bank_transaction')) {
      const sourceSystem = params[0] as string
      const sourceAccount = params[2] as string
      const fitid = params[6] as string
      const key = `${sourceSystem}:${sourceAccount}:${fitid}`
      if (rows.has(key)) return Promise.resolve([]) // conflict -> replayed
      const id = `id-${++n}`
      rows.set(key, { id, reconciliation_status: 'unmatched' })
      return Promise.resolve([{ id }])
    }
    return Promise.resolve([])
  }) as QueryExecutor
  return { execute, rows }
}

const baseInput = {
  sourceSystem: 'ofx_qbo',
  sourceFormat: 'QBO',
  sourceAccount: '0000123456789',
  bankId: '999999999',
  accountType: 'CHECKING',
  currencyCode: 'USD',
  sourceFileSha256: 'abc123',
}

test('first import inserts every transaction', async () => {
  const { execute } = makeStore()
  const counts = await insertBankTransactions(
    { ...baseInput, transactions: [tx({ fitid: 'A' }), tx({ fitid: 'B' }), tx({ fitid: 'C' })] },
    execute,
  )
  assert.deepEqual(counts, { inserted: 3, replayed: 0, rejected: 0 })
})

test('identical replay inserts zero duplicates', async () => {
  const { execute, rows } = makeStore()
  const input = {
    ...baseInput,
    transactions: [tx({ fitid: 'A' }), tx({ fitid: 'B' })],
  }
  const first = await insertBankTransactions(input, execute)
  assert.equal(first.inserted, 2)
  const second = await insertBankTransactions(input, execute)
  assert.deepEqual(second, { inserted: 0, replayed: 2, rejected: 0 })
  assert.equal(rows.size, 2) // no duplicate rows
})

test('overlapping file inserts only the new FITIDs', async () => {
  const { execute, rows } = makeStore()
  const first = await insertBankTransactions(
    { ...baseInput, transactions: [tx({ fitid: 'A' }), tx({ fitid: 'B' }), tx({ fitid: 'C' })] },
    execute,
  )
  assert.equal(first.inserted, 3)
  const overlapping = await insertBankTransactions(
    { ...baseInput, transactions: [tx({ fitid: 'B' }), tx({ fitid: 'C' }), tx({ fitid: 'D' })] },
    execute,
  )
  assert.deepEqual(overlapping, { inserted: 1, replayed: 2, rejected: 0 })
  assert.equal(rows.size, 4) // A, B, C, D
  assert.ok(rows.has('ofx_qbo:0000123456789:D'))
})

test('reconciliation state is not destroyed by replay', async () => {
  const { execute, rows } = makeStore()
  // First load inserts A.
  await insertBankTransactions({ ...baseInput, transactions: [tx({ fitid: 'A' })] }, execute)
  // Simulate an operator marking A matched.
  rows.set('ofx_qbo:0000123456789:A', { id: 'id-1', reconciliation_status: 'matched' })
  // Replay the same file.
  const replay = await insertBankTransactions(
    { ...baseInput, transactions: [tx({ fitid: 'A' })] },
    execute,
  )
  assert.deepEqual(replay, { inserted: 0, replayed: 1, rejected: 0 })
  // The existing row (with its reconciliation state) is untouched.
  assert.equal(rows.get('ofx_qbo:0000123456789:A')?.reconciliation_status, 'matched')
})

test('a transaction without FITID is rejected at the repository gate', async () => {
  const { execute, rows } = makeStore()
  const counts = await insertBankTransactions(
    { ...baseInput, transactions: [tx({ fitid: 'X' }), { ...tx({ fitid: 'Y' }), fitid: '' }] },
    execute,
  )
  assert.deepEqual(counts, { inserted: 1, replayed: 0, rejected: 1 })
  assert.equal(rows.size, 1)
})
