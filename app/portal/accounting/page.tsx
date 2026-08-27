import { AccountingDashboard } from '@/components/portal/accounting/accounting-dashboard'
import { AccountingShell } from '@/components/portal/accounting/accounting-shell'
import { getAccountingDashboard, getPnlStatement } from '@/db/accounting'

export const dynamic = 'force-dynamic'

// ACCOUNTING — Dashboard. Dark navy/gold executive summary over the canonical
// account_receivable / account_expense rows. All values are DB-derived; the P&L
// trend is derived from the same rows (never its own table). The bottom-band
// "Expenses by Category" reuses the existing P&L category aggregation
// (getPnlStatement) for the current month — no second aggregation model.
export default async function AccountingDashboardPage() {
  const now = new Date()
  const from = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-01`
  const lastDay = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate()
  const to = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(lastDay).padStart(2, '0')}`

  const [data, pnl] = await Promise.all([
    getAccountingDashboard(),
    getPnlStatement(from, to),
  ])

  return (
    <AccountingShell eyebrow="Accounting" title="Dashboard">
      <AccountingDashboard
        data={data}
        expenseCategories={pnl.expenses}
        expenseTotal={pnl.totalExpenses}
      />
    </AccountingShell>
  )
}
