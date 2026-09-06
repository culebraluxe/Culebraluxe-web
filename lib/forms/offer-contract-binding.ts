import { randomUUID } from 'node:crypto'

import { bindFormInstanceToContract, getFormContractId } from '@/db/contract-issued-document'
import { CONTRACT_OPERATIONS } from '@/services/contract'
import { formCoreServices } from './form-service-runtime'
import { formServiceContext, serviceValue } from './service-binding-core'
import { mapOfferContractDraft, type OfferContractDraft } from './offer-contract-mapping'

export async function syncOfferContractForm(
  form: OfferContractDraft,
  actorId: string | null,
): Promise<string> {
  const currentContractId = await getFormContractId(form.id)
  const contractId = currentContractId ?? randomUUID()
  const mapped = mapOfferContractDraft(form)

  const saved = await serviceValue(
    formCoreServices.contract.execute({
      operation: CONTRACT_OPERATIONS.SAVE_DRAFT,
      payload: {
        contractId,
        contractType: mapped.contractType,
        formTemplateId: 'OFFER-01',
        sourceFormInstanceId: form.id,
        predecessorContractId: null,
        propertyId: mapped.propertyId,
        roles: mapped.roles,
        facts: mapped.facts,
      },
      context: formServiceContext(actorId),
    }),
    'Offer Letter Contract binding failed',
  )

  await bindFormInstanceToContract({ formInstanceId: form.id, contractId: saved.id })
  return saved.id
}
