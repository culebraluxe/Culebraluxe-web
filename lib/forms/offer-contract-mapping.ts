import type { FormInstance } from '@/db/document-form-instance'
import type { ContractRoleDto } from '@/services/contract'
import { compactFormValue } from './service-binding-core'

const OFFER_FACT_FIELDS = [
  'buyerName',
  'sellerName',
  'brokerName',
  'property',
  'offerAmount',
  'deposit',
  'financing',
  'closingDate',
  'expiration',
  'contingencies',
] as const

export type OfferContractDraft = Pick<
  FormInstance,
  'id' | 'personId' | 'propertyId' | 'fieldValues' | 'sections'
>

export function mapOfferContractDraft(form: OfferContractDraft) {
  if (!form.personId || !form.propertyId) {
    throw new Error('OFFER-01 requires explicit Person and Property context.')
  }

  const facts: Record<string, string> = {}
  for (const field of OFFER_FACT_FIELDS) {
    facts[field] = compactFormValue(form.fieldValues[field])
  }
  facts.specialTerms = compactFormValue(form.sections.specialTerms)

  const roles: ContractRoleDto[] = [
    {
      kind: 'person',
      personId: form.personId,
      roleCode: 'BUYER',
      snapshotName: facts.buyerName || null,
    },
  ]

  return {
    contractType: 'offer_letter',
    propertyId: form.propertyId,
    roles,
    facts,
  }
}
