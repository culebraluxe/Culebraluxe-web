import { AccountingReceiptScanner } from '@/components/portal/accounting/accounting-receipt-scanner'
import { AccountingShell } from '@/components/portal/accounting/accounting-shell'

// ACCOUNTING — Receipt Scanner (FAKE V1). Polished visual placeholder for the
// future OCR workflow. Deterministic demo extraction only — real OCR is deferred
// to a separate story. No OCR vendor / AI / external service dependencies.
export default function AccountingReceiptScannerPage() {
  return (
    <AccountingShell eyebrow="Accounting" title="Receipt Scanner">
      <AccountingReceiptScanner />
    </AccountingShell>
  )
}
