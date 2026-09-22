import 'server-only'

import { endOfMonthISO, startOfMonthISO, todayISO } from '@/lib/accounting/format'
import { rustApiRead } from '@/lib/rust-api/client'

// ---------------------------------------------------------------------------
// ACCOUNTING PAYLOADS — one place, because two routes have to answer identically.
//
// The page route fetches a screen's data; the command route performs a write and then returns the REFRESHED payload so a
// command completion and a refresh are one round trip (the pattern Projects and Contracts already use). If those two
// built the payload separately they would eventually disagree about a field, and the disagreement would show up as a
// screen that loses data the moment somebody saves something.
//
// TYPES ARE MIRRORS, NOT RULES. These describe what the Rust service returns so the bridge can hand it on unopened; the
// shapes are `domain::accounting`'s, and MONEY IS A STRING on purpose — a decimal that arrives here as a JSON number has
// already been through a float, and the cent is already gone.
// ---------------------------------------------------------------------------

export type AccountingLine = { label: string; amount: string }

export type AccountingTrendPoint = {
  month: string
  income: string
  expenses: string
  net: string
}

export type AccountingAssociation = {
  dealId: string | null
  dealName: string | null
  propertyId: string | null
  propertyName: string | null
  personId: string | null
  personName: string | null
}

export type AccountingReceivable = AccountingAssociation & {
  id: string
  reference: string | null
  description: string
  category: string
  amount: string
  issuedOn: string
  dueOn: string | null
  status: string
  paidOn: string | null
}

export type AccountingExpense = AccountingAssociation & {
  id: string
  vendor: string
  category: string
  amount: string
  expenseOn: string
  status: string
  memo: string | null
}

export type AccountingDashboard = {
  receivablesOutstanding: string
  expensesThisMonth: string
  netIncome: string
  openCount: number
  overdueCount: number
  pnlTrend: AccountingTrendPoint[]
  recentExpenses: AccountingExpense[]
  recentActivity: AccountingReceivable[]
  expenseCategories: AccountingLine[]
}

export type AccountingPnl = {
  from: string
  to: string
  income: AccountingLine[]
  totalIncome: string
  expenses: AccountingLine[]
  totalExpenses: string
  netIncome: string
}

export type AccountingCategoryShare = AccountingLine & { percent: number }

/** The accounting screens, so a caller cannot ask this module for a screen it does not serve. */
export const ACCOUNTING_SCREENS = [
  'accounting',
  'accounting-expenses',
  'accounting-receivables',
  'accounting-pnl',
  'accounting-receipt-scanner',
] as const

export type AccountingScreen = (typeof ACCOUNTING_SCREENS)[number]

export function isAccountingScreen(value: string): value is AccountingScreen {
  return (ACCOUNTING_SCREENS as readonly string[]).includes(value)
}

/**
 * The payload one accounting screen needs, read from the Rust service.
 *
 * A screen is served exactly what it renders and nothing else: the two lists do not carry the dashboard's eight
 * projections, and the dashboard does not carry every row. The scanner needs no data at all — its demonstration receipts
 * are the screen's own, and inventing a read for it would be inventing a database nothing reads.
 */
export async function accountingPayload(
  screen: AccountingScreen,
  range: { from?: string | null; to?: string | null } = {},
): Promise<Record<string, unknown>> {
  switch (screen) {
    case 'accounting': {
      const result = await rustApiRead<AccountingDashboard>('/v1/accounting/dashboard')
      return { accounting: { dashboard: result.value } }
    }
    case 'accounting-expenses': {
      // The list, its breakdown, and today.
      //
      // THE BREAKDOWN IS THE SERVICE'S, NOT THE LIST'S. The live screen summed the rows it had already fetched — a float
      // sum of money computed in the browser, and a second aggregation that can disagree with the rows under it.
      //
      // `today` travels with the payload because the form's date field defaults to it. It is the same `todayISO()` the
      // server-rendered form used, so the default is unchanged; the alternative was a date the browser guesses, which for
      // an operator an hour from the server is a different day than the book's.
      const [expenses, categories] = await Promise.all([
        rustApiRead<AccountingExpense[]>('/v1/accounting/expenses'),
        rustApiRead<AccountingCategoryShare[]>('/v1/accounting/expense-categories'),
      ])
      return {
        accounting: {
          expenses: expenses.value,
          expenseCategories: categories.value,
          today: todayISO(),
        },
      }
    }
    case 'accounting-receivables': {
      // The list, plus the book's date: the create form's issue date and each row's paid date default to it, so a record
      // is dated by the book rather than by the browser.
      const receivables = await rustApiRead<AccountingReceivable[]>('/v1/accounting/receivables')
      return {
        accounting: { receivables: receivables.value, today: todayISO() },
      }
    }
    case 'accounting-pnl': {
      // THE PERIOD IS THE CALLER'S. A range that arrives is bound and validated in Rust; a range that does NOT arrive gets
      // the current month, which is the period the live page projected for an un-filtered visit. What never happens is a
      // silent substitution: the statement echoes back the period it covers, and the screen shows it.
      const from = range.from?.trim() || startOfMonthISO()
      const to = range.to?.trim() || endOfMonthISO()
      const result = await rustApiRead<AccountingPnl>(
        (`/v1/accounting/pnl?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`) as `/v1/${string}`,
      )
      return { accounting: { pnl: result.value } }
    }
    case 'accounting-receipt-scanner': {
      // No read: the scanner demonstrates the extraction workflow with its own deterministic receipts.
      return { accounting: {} }
    }
  }
}
