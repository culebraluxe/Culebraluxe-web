import { Panel } from '@/components/portal/panel'
import {
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from '@/components/portal/ui/portal-table'
import type { ContractPortfolioRow } from '@/lib/contract-reads'

/**
 * The artifact view beside the transaction view.
 *
 * The nav says *Contracts* while the page below lists Deals. A deal is the
 * transaction; a contract is its artifact. This panel makes the artifacts
 * visible through the Contract service without removing the transaction list —
 * so the page tells the truth today (7 transactions, 0 contracts) and keeps
 * telling it as contracts accumulate.
 */

function statusLabel(status: string) {
  return status
    .split(/[_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

function formatDate(value: string | null) {
  if (!value) return '—'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '—'
  return date.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' })
}

export function ContractsPortfolio({
  contracts,
  readFailed = false,
}: {
  contracts: ContractPortfolioRow[]
  readFailed?: boolean
}) {
  return (
    <Panel
      compact
      variant="standard"
      divider
      flush
      eyebrow="Artifacts"
      heading="Contracts from Forms"
    >
      {readFailed ? (
        <p className="px-6 py-8 text-sm font-light text-muted-foreground">
          The contract portfolio could not be read just now. The failure was captured — try again
          shortly.
        </p>
      ) : contracts.length === 0 ? (
        <p className="px-6 py-8 text-sm font-light text-muted-foreground">
          No contracts yet. One appears here as soon as a form creates it.
        </p>
      ) : (
        <div className="hidden md:block">
          <PortalTable>
            <PortalTableHead>
              <PortalTableRow className="hover:bg-transparent">
                <PortalTableHeader>Contract</PortalTableHeader>
                <PortalTableHeader>Type</PortalTableHeader>
                <PortalTableHeader>Property</PortalTableHeader>
                <PortalTableHeader>Status</PortalTableHeader>
                <PortalTableHeader>Transaction</PortalTableHeader>
                <PortalTableHeader>Executed</PortalTableHeader>
              </PortalTableRow>
            </PortalTableHead>
            <PortalTableBody>
              {contracts.map((contract) => (
                <PortalTableRow key={contract.id}>
                  <PortalTableCell>{contract.formTemplateId}</PortalTableCell>
                  <PortalTableCell>{contract.contractType}</PortalTableCell>
                  <PortalTableCell>{contract.propertyLabel ?? contract.propertyId}</PortalTableCell>
                  <PortalTableCell>{statusLabel(contract.status)}</PortalTableCell>
                  <PortalTableCell>{contract.processInstanceId ? 'Attached' : '—'}</PortalTableCell>
                  <PortalTableCell>{formatDate(contract.executedAt ?? contract.createdAt)}</PortalTableCell>
                </PortalTableRow>
              ))}
            </PortalTableBody>
          </PortalTable>
        </div>
      )}
    </Panel>
  )
}
