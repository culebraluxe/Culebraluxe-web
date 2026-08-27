import { AccountingPnl } from '@/components/portal/accounting/accounting-pnl'
import { AccountingShell } from '@/components/portal/accounting/accounting-shell'
import { getPnlStatement } from '@/db/accounting'
import { endOfMonthISO, startOfMonthISO } from '@/lib/accounting/format'

export const dynamic = 'force-dynamic'

// ACCOUNTING — P&L Statement. DERIVED from account_receivable / account_expense
// rows for the selected range; never persisted as its own model. Realized income
// (PAID receivables) vs posted expenses.
export default async function AccountingPnlPage({
  searchParams,
}: {
  searchParams: Promise<{ from?: string; to?: string }>
}) {
  const sp = await searchParams
  const from = sp.from ?? startOfMonthISO()
  const to = sp.to ?? endOfMonthISO()
  const statement = await getPnlStatement(from, to)
  return (
    <AccountingShell eyebrow="Accounting" title="P&L Statement">
      <AccountingPnl statement={statement} />
    </AccountingShell>
  )
}
