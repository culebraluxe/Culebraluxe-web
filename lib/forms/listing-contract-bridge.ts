import 'server-only'

import { randomUUID } from 'node:crypto'

import { coreServices } from '@/lib/service-runtime'
import { getActingUser } from '@/lib/auth/get-acting-user'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { resolveSecurityLevel } from '@/services/security'
import {
  CONTRACT_OPERATIONS,
  type ContractRoleDto,
} from '@/services/contract'
import type { FormInstance } from '@/services/forms'
import { captureServerError } from '@/lib/server-error-capture'
import { planContractFromForm } from './contract-from-form'
import { getTemplate } from './template-registry'

/**
 * The corridor: the FORM is the input, the CONTRACT is the artifact.
 *
 *   Forms ─────► Contract ─────► Vault
 *   (input)      (artifact)      (evidence)
 *
 * docs/REAL-ESTATE-TRANSACTION-DESIGN.md section 1. The mapper
 * (planContractFromForm) and the Contract operation (saveDraft / createFromForm)
 * already existed; this is the wiring that made the bridge real, and the
 * `document_form_instance.contract_id` column is the link it writes back.
 *
 * Contract type vocabulary is per template — `offer-contract-mapping` uses
 * 'offer_letter', so a Listing agreement is 'listing_agreement'.
 */

const service = coreServices.contract

const LISTING_CONTRACT_TYPE = 'listing_agreement'

async function portalContext() {
  const correlationId = randomUUID()
  try {
    const acting = await getActingUser(getPortalSessionAdapter())
    return {
      context: {
        actor: { id: acting.appUserId, kind: 'user' as const },
        correlationId,
        principal: {
          appUserId: acting.appUserId,
          level: resolveSecurityLevel(acting.roleCodes),
          roleCodes: acting.roleCodes,
        },
      },
      acting,
    }
  } catch {
    // No session: commands fail closed (the kernel refuses as GUEST).
    return {
      context: { actor: { id: null, kind: 'system' as const }, correlationId },
      acting: null,
    }
  }
}

/**
 * Make the form's Contract real, and return its id.
 *
 * Draft form      -> saveDraft (create or replace: saving twice updates one
 *                    contract instead of creating a second).
 * Issued form     -> createFromForm once if none exists yet (so a form issued
 *                    before this wiring still gets its artifact); an existing
 *                    contract on a non-draft form is never rewritten.
 *
 * Returns null when nothing was written (unknown template, no property, refused).
 */
export async function syncListingContract(form: FormInstance): Promise<string | null> {
  const template = getTemplate(form.templateId, form.templateVersion)
  if (!template) return null
  // A listing agreement is about a property; without one there is no subject.
  if (!form.propertyId) return null
  if (form.status !== 'draft' && form.contractId) return null

  const { context, acting } = await portalContext()
  const contractId = form.contractId ?? randomUUID()

  const parties: Record<string, Array<{ personId: string; snapshotName?: string | null }>> = {}
  if (form.personId) parties.SELLER = [{ personId: form.personId }]
  if (acting?.personId) {
    parties.SELLER_BROKER = [{ personId: acting.personId, snapshotName: acting.displayName }]
  }

  const plan = planContractFromForm({
    template,
    values: form.fieldValues,
    contractType: LISTING_CONTRACT_TYPE,
    contractId,
    propertyId: form.propertyId,
    parties,
    sourceFormInstanceId: form.id,
  })

  // A Contract Role needs a real identity. The template may declare roles the
  // operator has not filled yet (visible work on the form); they cannot be
  // written as contract roles until someone is chosen, so they are skipped here.
  const roles: ContractRoleDto[] = plan.roles
    .filter((role) => role.identityId)
    .map((role) =>
      role.kind === 'firm'
        ? {
            kind: 'firm' as const,
            firmId: role.identityId as string,
            roleCode: role.role,
            ordinal: role.ordinal,
            snapshotName: role.snapshotName,
          }
        : {
            kind: 'person' as const,
            personId: role.identityId as string,
            roleCode: role.role,
            ordinal: role.ordinal,
            snapshotName: role.snapshotName,
          },
    )

  const payload = {
    contractId,
    contractType: plan.contractType,
    formTemplateId: plan.formTemplateId,
    sourceFormInstanceId: plan.sourceFormInstanceId,
    predecessorContractId: plan.predecessorContractIds[0] ?? null,
    propertyId: plan.propertyId as string,
    roles,
    facts: plan.facts,
  }

  // Draft: create or replace (saving twice updates one artifact). An issued form
  // with no contract yet creates it once; an existing contract on a non-draft
  // form is never rewritten (that check runs earlier).
  const operation =
    form.status === 'draft'
      ? CONTRACT_OPERATIONS.SAVE_DRAFT
      : CONTRACT_OPERATIONS.CREATE_FROM_FORM

  const result = await service.execute({ operation, payload, context })
  if (!result.ok) {
    captureServerError('Listing contract bridge failed.', new Error(`${result.error.code}: ${result.error.message}`))
    return null
  }
  return contractId
}
