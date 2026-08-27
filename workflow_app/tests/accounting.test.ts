import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'
import {
  createExpense,
  createReceivable,
  getPnlStatement,
  markReceivablePaid,
} from '../../db/accounting'
import { EXPENSE_CATEGORIES, isExpenseCategory } from '../../lib/accounting/categories'
import { formatMoney, todayISO } from '../../lib/accounting/format'
import {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
  navigationForSurface,
  surfaceForPathname,
} from '../../lib/navigation'

// ACCOUNTING V1 — focused unit proof. Covers the new nav surface, the shared
// category/format helpers, and the canonical DB seams (validation + writes +
// derived P&L). Uses the DB test executor hook / explicit executor injection;
// never touches a real database.

afterEach(() => setDatabaseTestExecutor(null))

type Captured = { sql: string; params: unknown[] }

function makeExecutor(
  sequences: Record<string, unknown>[][],
  captured: Captured[],
): QueryExecutor {
  let i = 0
  return async (strings, ...params) => {
    captured.push({ sql: strings.join('?'), params })
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

// --- Navigation surface -----------------------------------------------------

test('accounting: is a top-level operating surface with exactly the 5 required items', () => {
  assert.ok(OPERATING_SURFACE_ORDER.includes('ACCOUNTING' as never))
  const def = OPERATING_SURFACES.ACCOUNTING
  assert.equal(def.home, '/portal/accounting')
  const items = navigationForSurface('ACCOUNTING' as never)
  const labels = items.map((i) => i.label)
  assert.deepEqual(labels, [
    'Dashboard',
    'Receivables',
    'Expenses',
    'P&L Statement',
    'Receipt Scanner',
  ])
  // Receipt Scanner must be the LAST item.
  assert.equal(items[items.length - 1].label, 'Receipt Scanner')
})

test('accounting: /portal/accounting routes resolve to the ACCOUNTING surface', () => {
  assert.equal(surfaceForPathname('/portal/accounting'), 'ACCOUNTING')
  assert.equal(
    surfaceForPathname('/portal/accounting/receivables'),
    'ACCOUNTING',
  )
  assert.equal(
    surfaceForPathname('/portal/accounting/receipt-scanner'),
    'ACCOUNTING',
  )
  // Existing surfaces are not disturbed.
  assert.equal(surfaceForPathname('/portal/dashboard'), 'NEXUS')
})

// --- Helpers ----------------------------------------------------------------

test('accounting: controlled expense category list', () => {
  assert.ok(EXPENSE_CATEGORIES.includes('Marketing & Advertising'))
  assert.ok(EXPENSE_CATEGORIES.includes('Property / Deal Expense'))
  assert.equal(EXPENSE_CATEGORIES.length, 9)
  assert.ok(isExpenseCategory('Insurance'))
  assert.ok(!isExpenseCategory('Random'))
})

test('accounting: format helpers', () => {
  assert.equal(formatMoney(1234.5), '$1,235')
  assert.equal(todayISO().length, 10)
})

// --- createReceivable -------------------------------------------------------

test('createReceivable: rejects a blank description before any query', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[]], captured)
  await assert.rejects(
    createReceivable(
      { description: '   ', category: 'COMMISSION', amount: 100, issuedOn: todayISO() },
      execute,
    ),
    /Description is required/,
  )
  assert.equal(captured.length, 0)
})

test('createReceivable: rejects a negative amount', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[]], captured)
  await assert.rejects(
    createReceivable(
      { description: 'Fee', category: 'COMMISSION', amount: -5, issuedOn: todayISO() },
      execute,
    ),
    /non-negative/,
  )
  assert.equal(captured.length, 0)
})

test('createReceivable: inserts trimmed description and uppercased category', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[{ id: 'r-1' }]], captured)
  const { id } = await createReceivable(
    {
      description: '  Commission on 14 Elm  ',
      category: 'misc_income',
      amount: 12500,
      issuedOn: '2026-08-01',
      dueOn: '2026-09-01',
    },
    execute,
  )
  assert.equal(id, 'r-1')
  assert.equal(captured.length, 1)
  const params = captured[0].params
  assert.equal(params[1], 'Commission on 14 Elm')
  assert.equal(params[2], 'MISC_INCOME')
})

// --- markReceivablePaid -----------------------------------------------------

test('markReceivablePaid: voids cannot be marked paid (conflict)', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[]], captured)
  await assert.rejects(
    markReceivablePaid('r-1', todayISO(), execute),
    /not found or voided/,
  )
})

test('markReceivablePaid: succeeds for an open receivable', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[{ id: 'r-1' }]], captured)
  const result = await markReceivablePaid('r-1', '2026-08-20', execute)
  assert.equal(result.status, 'PAID')
  assert.ok(/set status = 'PAID'/i.test(captured[0].sql))
  assert.equal(captured[0].params[0], '2026-08-20')
})

// --- createExpense ----------------------------------------------------------

test('createExpense: rejects an out-of-list category', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[]], captured)
  await assert.rejects(
    createExpense(
      { vendor: 'V', category: 'Random', amount: 10, expenseOn: todayISO() },
      execute,
    ),
    /category is invalid/,
  )
  assert.equal(captured.length, 0)
})

test('createExpense: inserts a valid posted expense', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor([[{ id: 'e-1' }]], captured)
  const { id } = await createExpense(
    {
      vendor: '  Metro Co  ',
      category: 'Office',
      amount: 99.5,
      expenseOn: '2026-08-02',
      memo: ' supplies ',
    },
    execute,
  )
  assert.equal(id, 'e-1')
  assert.equal(captured[0].params[0], 'Metro Co')
  assert.equal(captured[0].params[4], 'supplies')
})

// --- Derived P&L ------------------------------------------------------------

test('getPnlStatement: derives income and expense totals from rows (never its own table)', async () => {
  const captured: Captured[] = []
  const execute = makeExecutor(
    [
      [{ label: 'COMMISSION', amount: 10000 }],
      [
        { label: 'Office', amount: 500 },
        { label: 'Insurance', amount: 200 },
      ],
    ],
    captured,
  )
  const pnl = await getPnlStatement('2026-01-01', '2026-12-31', execute)
  assert.equal(pnl.totalIncome, 10000)
  assert.equal(pnl.totalExpenses, 700)
  assert.equal(pnl.netIncome, 9300)
  assert.equal(pnl.income.length, 1)
  assert.equal(pnl.expenses.length, 2)
  assert.equal(captured.length, 2)
  // both queries reference the accounting tables, never a P&L table
  assert.ok(/account_receivable/i.test(captured[0].sql))
  assert.ok(/account_expense/i.test(captured[1].sql))
})

