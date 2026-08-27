import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { PortalWriteError } from '../lib/portal-write-error'
import { isExpenseCategory } from '../lib/accounting/categories'

// ---------------------------------------------------------------------------
// ACCOUNTING V1 — DB read/write seams over the canonical DEV tables
// (public.account_receivable / public.account_expense).
//
// Reads join canonical Deal / Property / Person names (never UUIDs in the UI).
// P&L is DERIVED from these rows — it is never persisted as its own model.
// Writes are the single application seam (UI -> action -> here -> DB); the
// React client never touches SQL.
// ---------------------------------------------------------------------------

export type ReceivableStatus = 'OPEN' | 'PAID' | 'VOID'
export type ExpenseStatus = 'DRAFT' | 'POSTED' | 'VOID'

export type Receivable = {
  id: string
  reference: string | null
  description: string
  category: string
  amount: number
  issuedOn: string
  dueOn: string | null
  status: ReceivableStatus
  paidOn: string | null
  dealId: string | null
  dealName: string | null
  propertyId: string | null
  propertyName: string | null
  personId: string | null
  personName: string | null
}

export type Expense = {
  id: string
  vendor: string
  category: string
  amount: number
  expenseOn: string
  status: ExpenseStatus
  memo: string | null
  dealId: string | null
  dealName: string | null
  propertyId: string | null
  propertyName: string | null
  personId: string | null
  personName: string | null
}

export type PnlLine = { label: string; amount: number }

export type PnlStatement = {
  from: string
  to: string
  income: PnlLine[]
  totalIncome: number
  expenses: PnlLine[]
  totalExpenses: number
  netIncome: number
}

export type PnlTrendPoint = {
  month: string
  income: number
  expenses: number
  net: number
}

export type DashboardSummary = {
  receivablesOutstanding: number
  expensesThisMonth: number
  netIncome: number
  openCount: number
  overdueCount: number
  pnlTrend: PnlTrendPoint[]
  recentExpenses: Expense[]
  recentActivity: Receivable[]
}

const num = (v: unknown): number => Number(v ?? 0)

function toReceivable(r: Record<string, unknown>): Receivable {
  return {
    id: String(r.id),
    reference: (r.reference as string | null) ?? null,
    description: String(r.description ?? ''),
    category: String(r.category ?? ''),
    amount: num(r.amount),
    issuedOn: String(r.issued_on ?? ''),
    dueOn: (r.due_on as string | null) ?? null,
    status: (r.status as ReceivableStatus) ?? 'OPEN',
    paidOn: (r.paid_on as string | null) ?? null,
    dealId: (r.deal_id as string | null) ?? null,
    dealName: (r.deal_name as string | null) ?? null,
    propertyId: (r.property_id as string | null) ?? null,
    propertyName: (r.property_name as string | null) ?? null,
    personId: (r.person_id as string | null) ?? null,
    personName: (r.person_name as string | null) ?? null,
  }
}

function toExpense(r: Record<string, unknown>): Expense {
  return {
    id: String(r.id),
    vendor: String(r.vendor ?? ''),
    category: String(r.category ?? ''),
    amount: num(r.amount),
    expenseOn: String(r.expense_on ?? ''),
    status: (r.status as ExpenseStatus) ?? 'POSTED',
    memo: (r.memo as string | null) ?? null,
    dealId: (r.deal_id as string | null) ?? null,
    dealName: (r.deal_name as string | null) ?? null,
    propertyId: (r.property_id as string | null) ?? null,
    propertyName: (r.property_name as string | null) ?? null,
    personId: (r.person_id as string | null) ?? null,
    personName: (r.person_name as string | null) ?? null,
  }
}

export async function getReceivables(
  execute: QueryExecutor = sql,
): Promise<Receivable[]> {
  const rows = (await execute`
    select
      t.id, t.reference, t.description, t.category, t.amount,
      t.issued_on::text as issued_on, t.due_on::text as due_on,
      t.status, t.paid_on::text as paid_on,
      t.deal_id, t.property_id, t.person_id,
      person.display_name as person_name,
      property.name as property_name,
      deal_property.name as deal_name
    from account_receivable t
    left join person on person.id = t.person_id
    left join property on property.id = t.property_id
    left join deal on deal.id = t.deal_id
    left join property deal_property on deal_property.id = deal.property_id
    order by t.issued_on desc, t.created_at desc
  `) as Record<string, unknown>[]
  return rows.map(toReceivable)
}

export async function getExpenses(
  execute: QueryExecutor = sql,
): Promise<Expense[]> {
  const rows = (await execute`
    select
      t.id, t.vendor, t.category, t.amount, t.expense_on::text as expense_on,
      t.status, t.memo,
      t.deal_id, t.property_id, t.person_id,
      person.display_name as person_name,
      property.name as property_name,
      deal_property.name as deal_name
    from account_expense t
    left join person on person.id = t.person_id
    left join property on property.id = t.property_id
    left join deal on deal.id = t.deal_id
    left join property deal_property on deal_property.id = deal.property_id
    order by t.expense_on desc, t.created_at desc
  `) as Record<string, unknown>[]
  return rows.map(toExpense)
}

export async function getAccountingDashboard(
  execute: QueryExecutor = sql,
): Promise<DashboardSummary> {
  const [
    openRow,
    expRow,
    countsRow,
    netRow,
    trendIncome,
    trendExpense,
    recentExp,
    recentRec,
  ] = await Promise.all([
    execute`select coalesce(sum(amount),0) as v from account_receivable where status = 'OPEN'`,
    execute`select coalesce(sum(amount),0) as v from account_expense where status = 'POSTED' and date_trunc('month', expense_on) = date_trunc('month', current_date)`,
    execute`select
      count(*) filter (where status='OPEN' and (due_on is null or due_on >= current_date)) as open_count,
      count(*) filter (where status='OPEN' and due_on is not null and due_on < current_date) as overdue_count
      from account_receivable`,
    execute`select
      (select coalesce(sum(amount),0) from account_receivable where status='PAID') -
      (select coalesce(sum(amount),0) from account_expense where status='POSTED') as v`,
    execute`select to_char(date_trunc('month', paid_on), 'YYYY-MM') as month, coalesce(sum(amount),0) as amount
      from account_receivable where status='PAID' and paid_on >= (current_date - interval '5 months')
      group by 1`,
    execute`select to_char(date_trunc('month', expense_on), 'YYYY-MM') as month, coalesce(sum(amount),0) as amount
      from account_expense where status='POSTED' and expense_on >= (current_date - interval '5 months')
      group by 1`,
    execute`select t.id, t.vendor, t.category, t.amount, t.expense_on::text as expense_on, t.status, t.memo,
      person.display_name as person_name, property.name as property_name, deal_property.name as deal_name
      from account_expense t
      left join person on person.id = t.person_id
      left join property on property.id = t.property_id
      left join deal on deal.id = t.deal_id
      left join property deal_property on deal_property.id = deal.property_id
      where t.status = 'POSTED' order by t.expense_on desc limit 5`,
    execute`select t.id, t.reference, t.description, t.category, t.amount, t.issued_on::text as issued_on, t.due_on::text as due_on, t.status,
      person.display_name as person_name, property.name as property_name, deal_property.name as deal_name
      from account_receivable t
      left join person on person.id = t.person_id
      left join property on property.id = t.property_id
      left join deal on deal.id = t.deal_id
      left join property deal_property on deal_property.id = deal.property_id
      where t.status <> 'VOID' order by t.issued_on desc limit 6`,
  ])

  const incomeByMonth = new Map<string, number>()
  for (const row of trendIncome as Array<{ month: string; amount: unknown }>) {
    incomeByMonth.set(row.month, num(row.amount))
  }
  const expenseByMonth = new Map<string, number>()
  for (const row of trendExpense as Array<{ month: string; amount: unknown }>) {
    expenseByMonth.set(row.month, num(row.amount))
  }

  const now = new Date()
  const pnlTrend: PnlTrendPoint[] = []
  for (let i = 5; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth() - i, 1)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`
    const income = incomeByMonth.get(key) ?? 0
    const expenses = expenseByMonth.get(key) ?? 0
    pnlTrend.push({
      month: d.toLocaleDateString(undefined, { month: 'short' }),
      income,
      expenses,
      net: income - expenses,
    })
  }

  return {
    receivablesOutstanding: num((openRow[0] as { v?: unknown })?.v),
    expensesThisMonth: num((expRow[0] as { v?: unknown })?.v),
    netIncome: num((netRow[0] as { v?: unknown })?.v),
    openCount: num((countsRow[0] as { open_count?: unknown })?.open_count),
    overdueCount: num((countsRow[0] as { overdue_count?: unknown })?.overdue_count),
    pnlTrend,
    recentExpenses: (recentExp as Record<string, unknown>[]).map(toExpense),
    recentActivity: (recentRec as Record<string, unknown>[]).map(toReceivable),
  }
}

export async function getPnlStatement(
  from: string,
  to: string,
  execute: QueryExecutor = sql,
): Promise<PnlStatement> {
  const [incomeRows, expenseRows] = await Promise.all([
    execute`select category as label, coalesce(sum(amount),0) as amount
      from account_receivable where status = 'PAID' and paid_on between ${from}::date and ${to}::date
      group by category order by amount desc`,
    execute`select category as label, coalesce(sum(amount),0) as amount
      from account_expense where status = 'POSTED' and expense_on between ${from}::date and ${to}::date
      group by category order by amount desc`,
  ])
  const income = (incomeRows as Array<{ label: string; amount: unknown }>).map(
    (r) => ({ label: r.label, amount: num(r.amount) }),
  )
  const expenses = (expenseRows as Array<{ label: string; amount: unknown }>).map(
    (r) => ({ label: r.label, amount: num(r.amount) }),
  )
  const totalIncome = income.reduce((s, l) => s + l.amount, 0)
  const totalExpenses = expenses.reduce((s, l) => s + l.amount, 0)
  return {
    from,
    to,
    income,
    totalIncome,
    expenses,
    totalExpenses,
    netIncome: totalIncome - totalExpenses,
  }
}


export async function createReceivable(
  input: {
    reference?: string | null
    description: string
    category: string
    amount: number
    issuedOn: string
    dueOn?: string | null
    dealId?: string | null
    propertyId?: string | null
    personId?: string | null
  },
  execute: QueryExecutor = sql,
): Promise<{ id: string }> {
  const description = input.description.trim()
  const amount = input.amount
  if (!description) {
    throw new PortalWriteError('validation', 'Description is required.')
  }
  if (!Number.isFinite(amount) || amount < 0) {
    throw new PortalWriteError('validation', 'Amount must be a non-negative number.')
  }
  const rows = await execute`
    insert into account_receivable (
      reference, description, category, amount, issued_on, due_on, deal_id, property_id, person_id
    ) values (
      ${input.reference?.trim() || null},
      ${description},
      ${input.category.trim().toUpperCase() || 'COMMISSION'},
      ${amount},
      ${input.issuedOn},
      ${input.dueOn ?? null},
      ${input.dealId ?? null},
      ${input.propertyId ?? null},
      ${input.personId ?? null}
    ) returning id
  `
  const id = (rows[0] as { id?: string } | undefined)?.id
  if (!id) throw new Error('Receivable insert did not return a row.')
  return { id }
}

export async function markReceivablePaid(
  id: string,
  paidOn: string,
  execute: QueryExecutor = sql,
): Promise<{ id: string; status: 'PAID' }> {
  const rows = await execute`
    update account_receivable
    set status = 'PAID', paid_on = ${paidOn}, updated_at = now()
    where id = ${id} and status <> 'VOID'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('conflict', 'Receivable not found or voided.')
  }
  return { id, status: 'PAID' }
}

export async function createExpense(
  input: {
    vendor: string
    category: string
    amount: number
    expenseOn: string
    memo?: string | null
    dealId?: string | null
    propertyId?: string | null
    personId?: string | null
  },
  execute: QueryExecutor = sql,
): Promise<{ id: string }> {
  const vendor = input.vendor.trim()
  const amount = input.amount
  if (!vendor) {
    throw new PortalWriteError('validation', 'Vendor is required.')
  }
  if (!isExpenseCategory(input.category)) {
    throw new PortalWriteError('validation', 'Expense category is invalid.')
  }
  if (!Number.isFinite(amount) || amount < 0) {
    throw new PortalWriteError('validation', 'Amount must be a non-negative number.')
  }
  const rows = await execute`
    insert into account_expense (
      vendor, category, amount, expense_on, memo, deal_id, property_id, person_id
    ) values (
      ${vendor},
      ${input.category},
      ${amount},
      ${input.expenseOn},
      ${input.memo?.trim() || null},
      ${input.dealId ?? null},
      ${input.propertyId ?? null},
      ${input.personId ?? null}
    ) returning id
  `
  const id = (rows[0] as { id?: string } | undefined)?.id
  if (!id) throw new Error('Expense insert did not return a row.')
  return { id }
}

