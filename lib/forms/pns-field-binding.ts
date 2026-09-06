import type { TemplateDefinition } from './template-types'
import type { RoleScope } from '@/services/core/role'

export type PnsFieldOwner = 'relation' | 'property' | 'contract'
export type PnsBindingReadiness = 'clean' | 'projection_pressure' | 'adapter_gap'
export type PnsRelationTarget = 'person' | 'firm' | 'property'

export type PnsRelationOption = {
  scope: RoleScope
  roleCode: string
  target: PnsRelationTarget
}

export type PnsFieldBinding = {
  field: string
  owner: PnsFieldOwner
  path: string
  readiness: PnsBindingReadiness
  relations?: readonly PnsRelationOption[]
  note?: string
}

const relation = (
  field: string,
  path: string,
  relations: readonly PnsRelationOption[],
  note?: string,
  readiness: PnsBindingReadiness = 'clean',
): PnsFieldBinding => ({ field, owner: 'relation', path, relations, note, readiness })

const property = (
  field: string,
  path: string,
  readiness: PnsBindingReadiness = 'clean',
  note?: string,
): PnsFieldBinding => ({ field, owner: 'property', path, readiness, note })

const contract = (
  field: string,
  path: string,
  note?: string,
  readiness: PnsBindingReadiness = 'clean',
): PnsFieldBinding => ({ field, owner: 'contract', path, readiness, note })

/**
 * Exhaustive ownership map for active PR-PNS v3.
 *
 * No Deal ownership is allowed here. Existing deal.* template sources are
 * treated as compatibility inputs only and are surfaced by the sidecar as
 * legacy bindings that need to be retired once Contract persistence is wired.
 */
export const PNS_FIELD_BINDINGS: readonly PnsFieldBinding[] = [
  relation('buyerName', 'Contract.role(BUYER).displayName', [
    { scope: 'contract_person', roleCode: 'BUYER', target: 'person' },
    { scope: 'contract_firm', roleCode: 'BUYER', target: 'firm' },
  ], 'Buyer is a Contract role; the legal party may be a Person or Firm.', 'projection_pressure'),
  relation('sellerName', 'Contract.role(SELLER).displayName', [
    { scope: 'contract_person', roleCode: 'SELLER', target: 'person' },
    { scope: 'contract_firm', roleCode: 'SELLER', target: 'firm' },
  ], 'Seller is a Contract role; the legal party may be a Person or Firm.', 'projection_pressure'),
  relation('buyerBrokerName', 'Contract.role(BUYER_BROKER).displayName', [
    { scope: 'contract_person', roleCode: 'BUYER_BROKER', target: 'person' },
    { scope: 'contract_firm', roleCode: 'BUYER_BROKERAGE', target: 'firm' },
  ], 'One presentation field currently conflates the broker Person and brokerage Firm.', 'projection_pressure'),
  relation('sellerBrokerName', 'Contract.role(SELLER_BROKER).displayName', [
    { scope: 'contract_person', roleCode: 'SELLER_BROKER', target: 'person' },
    { scope: 'contract_firm', roleCode: 'SELLER_BROKERAGE', target: 'firm' },
  ], 'One presentation field currently conflates the broker Person and brokerage Firm.', 'projection_pressure'),
  relation('spouseName', 'Contract.role(SELLER_SPOUSE).displayName', [
    { scope: 'contract_person', roleCode: 'SELLER_SPOUSE', target: 'person' },
  ], 'Discovery may follow Person↔Person SPOUSE, but this field projects the spouse role on this Contract.'),

  property('property', 'Property.localName', 'clean', 'Marketing/local name only; the recorded legal description remains controlling.'),
  property('municipality', 'Property.address.city'),
  property('catastroNumber', 'Property.catastroNumber'),
  property('registryEntry', 'Property.registryEntry', 'adapter_gap', 'Migration 117 adds the canonical column; PropertyService DTO/repository exposure still needs wiring.'),
  property('fincaNumber', 'Property.fincaNumber', 'adapter_gap', 'Migration 117 adds the canonical column; PropertyService DTO/repository exposure still needs wiring.'),
  property('registrySection', 'Property.registrySection', 'adapter_gap', 'Migration 117 adds the canonical column; PropertyService DTO/repository exposure still needs wiring.'),

  contract('purchasePrice', 'Contract.terms.purchasePrice'),
  contract('deposit', 'Contract.terms.deposit'),
  contract('cashAtClosing', 'Contract.terms.cashAtClosing'),
  contract('closingDate', 'Contract.terms.closingDate'),
  relation('escrowHolder', 'Contract.role(ESCROW_HOLDER).displayName', [
    { scope: 'contract_firm', roleCode: 'ESCROW_HOLDER', target: 'firm' },
  ], 'The Contract must still snapshot the exact rendered name even when a Firm identity is resolved.', 'projection_pressure'),
  relation('notaryName', 'Contract.role(CLOSING_NOTARY).displayName', [
    { scope: 'contract_person', roleCode: 'CLOSING_NOTARY', target: 'person' },
  ], 'Notary is a Person identity with a Contract role; rendered name remains an immutable Contract snapshot.', 'projection_pressure'),
  contract('effectiveDate', 'Contract.terms.effectiveDate'),

  contract('financing', 'Contract.terms.financing', '"Show All" looks presentation-oriented and should not become a durable financing value without an explicit decision.', 'projection_pressure'),
  contract('financingDeadline', 'Contract.terms.financingDeadline'),
  relation('lenderName', 'Contract.role(LENDER).displayName', [
    { scope: 'contract_firm', roleCode: 'LENDER', target: 'firm' },
  ], 'Lender is normally a Firm; the Contract snapshots the legal/rendered lender name.', 'projection_pressure'),
  contract('bankLoanAmount', 'Contract.terms.bankLoanAmount'),
  contract('ownerPrincipal', 'Contract.terms.ownerPrincipal'),
  contract('ownerRate', 'Contract.terms.ownerRate'),
  contract('ownerMaturity', 'Contract.terms.ownerMaturity'),

  contract('appraisalWaived', 'Contract.terms.appraisalWaived'),
  contract('surveyDeadline', 'Contract.terms.surveyDeadline'),
  contract('inspectionDeadline', 'Contract.terms.inspectionDeadline'),
  contract('sellerCapacity', 'Contract.role(SELLER).capacity', 'Capacity belongs to the Seller role in this Contract, not to Person.'),
  contract('entitySignerTitle', 'Contract.role(SELLER).signerTitle', 'Signer title is contextual to the entity representation in this Contract.'),
  contract('poderDate', 'Contract.role(SELLER).powerOfAttorneyDate', 'Power-of-attorney evidence is a Contract-scoped assertion.'),
  contract('sellerPRResident', 'Contract.role(SELLER).prResidentAssertion', 'Legal/tax assertion made for this Contract; do not silently promote to Person truth.'),
  contract('sellerFirptaForeign', 'Contract.role(SELLER).firptaForeignAssertion', 'Legal/tax assertion made for this Contract; do not silently promote to Person truth.'),
  contract('brokerRepresentation', 'Contract.terms.brokerRepresentation'),
] as const

const BINDING_BY_FIELD = new Map(PNS_FIELD_BINDINGS.map((binding) => [binding.field, binding]))

export function getPnsFieldBinding(fieldName: string): PnsFieldBinding | null {
  return BINDING_BY_FIELD.get(fieldName) ?? null
}

export type PnsBindingAudit = {
  templateFieldCount: number
  mappedFieldCount: number
  orphanFields: readonly string[]
  staleBindings: readonly string[]
  legacyDealFields: readonly string[]
  adapterGapFields: readonly string[]
  projectionPressureFields: readonly string[]
}

export function auditPnsFieldBindings(template: TemplateDefinition): PnsBindingAudit {
  const templateNames = new Set(template.fields.map((field) => field.name))
  const orphanFields = template.fields
    .filter((field) => !BINDING_BY_FIELD.has(field.name))
    .map((field) => field.name)
  const staleBindings = PNS_FIELD_BINDINGS
    .filter((binding) => !templateNames.has(binding.field))
    .map((binding) => binding.field)
  const mapped = template.fields
    .map((field) => BINDING_BY_FIELD.get(field.name))
    .filter((binding): binding is PnsFieldBinding => Boolean(binding))

  return {
    templateFieldCount: template.fields.length,
    mappedFieldCount: mapped.length,
    orphanFields,
    staleBindings,
    legacyDealFields: template.fields
      .filter((field) => field.binding?.startsWith('deal.'))
      .map((field) => field.name),
    adapterGapFields: mapped
      .filter((binding) => binding.readiness === 'adapter_gap')
      .map((binding) => binding.field),
    projectionPressureFields: mapped
      .filter((binding) => binding.readiness === 'projection_pressure')
      .map((binding) => binding.field),
  }
}
