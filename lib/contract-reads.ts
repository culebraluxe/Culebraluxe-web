import 'server-only'

import {
  rustApiContracts,
  rustApiPropertyRef,
  type RustContractSummary,
} from '@/lib/rust-api/client'

export type ContractSummaryDto = {
  id: string
  contractType: string
  formTemplateId: string
  status: string
  propertyId: string
  predecessorContractId: string | null
  processInstanceId: string | null
  evidenceDocumentId: string | null
  executedAt: string | null
  createdAt: string
}

export type ContractPortfolioRow = ContractSummaryDto & {
  propertyLabel: string | null
}

type ReadResult<T> =
  | { ok: true; data: T }
  | {
      ok: false
      error: {
        kind: 'UNKNOWN'
        operation: string
        incidentId: string
        code: string
        detail: string
      }
    }

function mapContract(row: RustContractSummary): ContractSummaryDto {
  return {
    id: row.id,
    contractType: row.contract_type,
    formTemplateId: row.form_template_id,
    status: row.status,
    propertyId: row.property_id,
    predecessorContractId: row.predecessor_contract_id,
    processInstanceId: row.process_instance_id,
    evidenceDocumentId: row.evidence_document_id,
    executedAt: row.executed_at,
    createdAt: row.created_at,
  }
}

/**
 * Contracts portfolio backed entirely by Rust services. Property labels are
 * composed from Rust's canonical Property read; authorization remains in Rust.
 */
export async function listContractPortfolio(): Promise<ReadResult<ContractPortfolioRow[]>> {
  try {
    const raw = await rustApiContracts()
    const contracts = raw.map(mapContract)
    const propertyIds = [...new Set(contracts.map((row) => row.propertyId).filter(Boolean))]
    const labels = new Map<string, string | null>()

    await Promise.all(
      propertyIds.map(async (propertyId) => {
        try {
          const property = await rustApiPropertyRef(propertyId)
          if (!property) return
          labels.set(
            propertyId,
            property.display_name ||
              property.local_name ||
              property.address_line1 ||
              property.municipality ||
              null,
          )
        } catch {
          // Preserve the row even when a secondary label lookup fails.
        }
      }),
    )

    return {
      ok: true,
      data: contracts.map((row) => ({
        ...row,
        propertyLabel: labels.get(row.propertyId) ?? null,
      })),
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : 'Contract portfolio read failed.'
    return {
      ok: false,
      error: {
        kind: 'UNKNOWN',
        operation: 'contract.list',
        incidentId: 'rust-contract-read',
        code: 'RUST_API_FAILURE',
        detail,
      },
    }
  }
}
