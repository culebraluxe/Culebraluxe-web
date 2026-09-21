import { AccountingExpenses } from '@/components/portal/accounting/accounting-expenses'
import { AccountingShell } from '@/components/portal/accounting/accounting-shell'
import { getExpenses } from '@/db/accounting'

export const dynamic = 'force-dynamic'

// ACCOUNTING — Expenses. Practical expense grid + "where is the money going?"
// category breakdown over account_expense (controlled category list).
export default async function AccountingExpensesPage() {
  const rows = await getExpenses()
  return (
    <AccountingShell eyebrow="Accounting" title="Expenses">
      <AccountingExpenses rows={rows} />
    </AccountingShell>
  )
}
