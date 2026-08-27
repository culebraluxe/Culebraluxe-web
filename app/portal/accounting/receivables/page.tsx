import { AccountingReceivables } from '@/components/portal/accounting/accounting-receivables'
import { AccountingShell } from '@/components/portal/accounting/accounting-shell'
import { getReceivables } from '@/db/accounting'

export const dynamic = 'force-dynamic'

// ACCOUNTING — Receivables. CRUD-lite operating screen over account_receivable.
// Canonical Deal / Property / Person names are shown (never UUIDs).
export default async function AccountingReceivablesPage() {
  const rows = await getReceivables()
  return (
    <AccountingShell eyebrow="Accounting" title="Receivables">
      <AccountingReceivables rows={rows} />
    </AccountingShell>
  )
}
