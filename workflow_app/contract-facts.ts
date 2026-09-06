import { randomUUID } from 'node:crypto'

import { sql } from '../db/client'
import { SqlContractRepository } from '../db/contract-service-repository'
import { SqlPropertyRepository } from '../db/property-service-repository'
import { isPnsTbd } from '../lib/forms/pns-canonical-types'
import { ContractService, CONTRACT_OPERATIONS, type ContractDto } from '../services/contract'
import { PropertyService, PROPERTY_OPERATIONS } from '../services/property'
import { getDealWorkflowFacts, type DealWorkflowFacts } from './facts'

// ---------------------------------------------------------------------------
// WORKFLOW-CONTRACT-01 — Contract-first workflow facts.
//
// Contract is the authority for P&S agreement truth. Deal remains a temporary
// compatibility projection for facts/commands that have not been strangled out
// yet. The merge order is intentional:
//
//   legacy Deal facts -> Contract facts -> normalized Contract decision facts
//
// A Contract value of TBD means unresolved and therefore becomes null at the
// workflow decision boundary; Workflow never treats TBD as a real date/amount/
// boolean. The literal TBD remains safely preserved in the mutable Contract
// draft for the broker to fix on a later pass.
// ---------------------------------------------------------------------------

const contractService = new ContractService(new SqlContractRepository())
const propertyService = new PropertyService(new SqlPropertyRepository())

type LegacyDealLinkRow = { deal_id: string | null }

export type ContractWorkflowFacts = Record<string, unknown> & {
  contractId: string
  contractStatus: string
  contractExecuted: boolean
  legacyDealId: string | null
  propertyId: string
}

function serviceContext() {
  return {
    actor: { id: null, kind: 'system' as const },
    correlationId: randomUUID(),
  }
}

async function serviceValue<T>(
  promise: Promise<{ ok: true; value: T } | { ok: false; error: { code: string; message: string } }>,
  label: string,
): Promise<T> {
  const result = await promise
  if (!result.ok) throw new Error(`${label}: ${result.error.code} ${result.error.message}`)
  return result.value
}

function hasFact(contract: ContractDto, name: string): boolean {
  return Object.prototype.hasOwnProperty.call(contract.facts, name)
}

function resolvedText(contract: ContractDto, name: string): string | null {
  const raw = contract.facts[name]
  if (raw === null || raw === undefined || isPnsTbd(raw)) return null
  if (typeof raw !== 'string' && typeof raw !== 'number' && typeof raw !== 'boolean') return null
  const value = String(raw).trim()
  return value ? value : null
}

function money(contract: ContractDto, name: string): number | null {
  const text = resolvedText(contract, name)
  if (!text) return null
  const normalized = text.replace(/[$,\s]/g, '')
  const value = Number(normalized)
  return Number.isFinite(value) ? value : null
}

function yesNo(contract: ContractDto, name: string): boolean | null {
  const text = resolvedText(contract, name)?.toLowerCase()
  if (!text) return null
  if (text === 'yes' || text === 'true') return true
  if (text === 'no' || text === 'false') return false
  return null
}

function financingApplicable(contract: ContractDto): boolean | null {
  const financing = resolvedText(contract, 'financing')?.toLowerCase()
  if (!financing) return null
  if (financing === 'cash') return false
  if (financing === 'bank' || financing === 'owner' || financing === 'blend' || financing === 'financed') {
    return true
  }
  return null
}

/**
 * Temporary strangler correlation. A Contract created from the existing Form
 * can still reach the old Deal command surface through the Form's deal_id.
 * No property/person heuristic is used: an ambiguous Contract must fail rather
 * than silently command the wrong Deal.
 */
export async function resolveLegacyDealIdForContract(contractId: string): Promise<string | null> {
  const rows = (await sql`
    select f.deal_id
    from contract c
    left join document_form_instance f on f.id = c.source_form_instance_id
    where c.id = ${contractId}
    limit 1
  `) as LegacyDealLinkRow[]
  return rows[0]?.deal_id ?? null
}

export async function getContractWorkflowFacts(
  contractId: string,
): Promise<ContractWorkflowFacts | null> {
  const contract = await serviceValue(
    contractService.execute({
      operation: CONTRACT_OPERATIONS.GET,
      payload: { contractId },
      context: serviceContext(),
    }),
    'Contract workflow lookup failed',
  )
  if (!contract) return null

  const legacyDealId = await resolveLegacyDealIdForContract(contract.id)
  const legacy: DealWorkflowFacts | null = legacyDealId
    ? await getDealWorkflowFacts(legacyDealId)
    : null

  const property = await serviceValue(
    propertyService.execute({
      operation: PROPERTY_OPERATIONS.GET,
      payload: { propertyId: contract.propertyId },
      context: serviceContext(),
    }),
    'Contract Property workflow lookup failed',
  )

  const closingDate = hasFact(contract, 'closingDate')
    ? resolvedText(contract, 'closingDate')
    : legacy?.closingDate ?? null
  const inspectionDeadline = hasFact(contract, 'inspectionDeadline')
    ? resolvedText(contract, 'inspectionDeadline')
    : legacy?.inspectionDeadline ?? null
  const financingDeadline = hasFact(contract, 'financingDeadline')
    ? resolvedText(contract, 'financingDeadline')
    : legacy?.financingDeadline ?? null

  const contractFinancingApplicable = hasFact(contract, 'financing')
    ? financingApplicable(contract)
    : legacy?.financingApplicable ?? null

  const waived = hasFact(contract, 'appraisalWaived')
    ? yesNo(contract, 'appraisalWaived')
    : null
  const appraisalApplicable = hasFact(contract, 'appraisalWaived')
    ? waived === null ? null : !waived
    : legacy?.appraisalApplicable ?? null

  const purchasePrice = hasFact(contract, 'purchasePrice')
    ? money(contract, 'purchasePrice')
    : legacy?.offerPrice ?? null

  const sameLegacyProperty = legacy?.property?.id === contract.propertyId

  return {
    ...(legacy ?? {}),
    ...contract.facts,

    // Contract identity/lifecycle is now first-class workflow context.
    contractId: contract.id,
    contractStatus: contract.status,
    contractExecuted: contract.status === 'executed' || contract.executedAt !== null,
    contractExecutedAt: contract.executedAt,
    contractEvidenceDocumentId: contract.evidenceDocumentId,
    legacyDealId,

    // Contract owns the subject Property even while Deal remains a compatibility
    // projection for unrelated pre/post-contract workflow facts.
    propertyId: contract.propertyId,
    property: property
      ? {
          id: property.id,
          name: property.localName ?? property.displayName,
          propertyType: sameLegacyProperty ? legacy?.property?.propertyType ?? null : null,
          status: property.status,
        }
      : null,

    // P&S economics and contingency decisions come from Contract, not Deal.
    purchasePrice,
    offerPrice: purchasePrice,
    closingDate,
    closingDateScheduled: closingDate !== null,
    inspectionDeadline,
    inspectionDeadlineScheduled: inspectionDeadline !== null,
    financingDeadline,
    financingDeadlineScheduled: financingDeadline !== null,
    financingApplicable: contractFinancingApplicable,
    appraisalApplicable,
  }
}
