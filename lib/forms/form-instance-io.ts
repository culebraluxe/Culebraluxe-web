import 'server-only'

import { randomUUID } from 'node:crypto'

import { PortalWriteError } from '@/lib/portal-write-error'
import { captureServerError } from '@/lib/server-error-capture'
import { coreServices } from '@/lib/service-runtime'
import { getActingUser } from '@/lib/auth/get-acting-user'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { resolveSecurityLevel } from '@/services/security'
import { FORM_OPERATIONS } from '@/services/forms'
import type {
  CreateFormInstanceInput,
  DealFormFacts,
  FormInstance,
  FormInstanceListItem,
  UpdateFormInstanceInput,
} from '@/services/forms'
import type { FormSignerPerson } from '@/lib/forms/signer-resolution'

/**
 * Form instance I/O for server surfaces — through the FORMS SERVICE, never the
 * database. (The retired db/document-form-instance.ts was form persistence living
 * outside the Forms domain.)
 *
 * The call shape matches what the Forms page/actions already expect, so the move
 * is an import change: validation failures still throw PortalWriteError (which the
 * actions map to 'validation'/'not-found'), and missing rows stay null.
 */

export type {
  CreateFormInstanceInput,
  DealFormFacts,
  FormInstance,
  FormInstanceListItem,
  FormInstanceStatus,
  UpdateFormInstanceInput,
} from '@/services/forms'

const service = coreServices.form

/** The Forms domain is always composed by the full runtime; fail loudly if not. */
function formService() {
  if (!service) throw new Error('Forms service is not composed in this runtime.')
  return service
}

/** The acting operator, so form commands are authorized as that user. */
async function serviceContext() {
  const correlationId = randomUUID()
  try {
    const acting = await getActingUser(getPortalSessionAdapter())
    return {
      actor: { id: acting.appUserId, kind: 'user' as const },
      correlationId,
      principal: {
        appUserId: acting.appUserId,
        level: resolveSecurityLevel(acting.roleCodes),
        roleCodes: acting.roleCodes,
      },
    }
  } catch {
    // No session (background/route context): reads still work, commands fail closed.
    return { actor: { id: null, kind: 'system' as const }, correlationId }
  }
}

async function run<T>(operation: string, payload: unknown, label: string): Promise<T> {
  const result = await formService().execute({
    operation: operation as never,
    payload: payload as never,
    context: await serviceContext(),
  })
  if (!result.ok) {
    captureServerError(label, new Error(`${result.error.code}: ${result.error.message}`), {
      level: 'error',
    })
    // Not a PortalWriteError: the actions classify anything else as 'unknown'.
    throw new Error(result.error.message)
  }
  return result.value as T
}

/** The same bounded validation the repository enforces, so the UI keeps its codes. */
function validateCreate(input: CreateFormInstanceInput): void {
  if (!input.templateId.trim()) {
    throw new PortalWriteError('validation', 'templateId is required.')
  }
  const dealId = input.dealId?.trim() || null
  const personId = input.personId?.trim() || null
  const propertyId = input.propertyId?.trim() || null
  if (!dealId && !personId && !propertyId) {
    throw new PortalWriteError('validation', 'A deal, client, or property is required.')
  }
}

export async function createFormInstance(input: CreateFormInstanceInput): Promise<FormInstance> {
  validateCreate(input)
  return run<FormInstance>(FORM_OPERATIONS.CREATE_INSTANCE, input, 'forms:createInstance')
}

export async function getFormInstance(id: string): Promise<FormInstance | null> {
  return run<FormInstance | null>(
    FORM_OPERATIONS.GET_INSTANCE,
    { formInstanceId: id },
    'forms:getInstance',
  )
}

export async function updateFormInstance(
  id: string,
  input: UpdateFormInstanceInput,
): Promise<FormInstance | null> {
  return run<FormInstance | null>(
    FORM_OPERATIONS.UPDATE_INSTANCE,
    { formInstanceId: id, input },
    'forms:updateInstance',
  )
}

export async function listFormInstances(): Promise<FormInstanceListItem[]> {
  return run<FormInstanceListItem[]>(FORM_OPERATIONS.LIST_INSTANCES, {}, 'forms:listInstances')
}

export async function getDealFormFacts(dealId: string): Promise<DealFormFacts | null> {
  return run<DealFormFacts | null>(FORM_OPERATIONS.DEAL_FACTS, { dealId }, 'forms:dealFacts')
}

export async function seedFormParticipantsFromDeal(
  formInstanceId: string,
  dealId: string,
): Promise<void> {
  await run<void>(
    FORM_OPERATIONS.SEED_PARTICIPANTS,
    { formInstanceId, dealId },
    'forms:seedParticipants',
  )
}

/* ------------------------------------------------------------------ *
 * Lineage — what the instance is bound to.
 * ------------------------------------------------------------------ */

export async function resolveDealLaunchContext(
  dealId: string,
): Promise<{ personId: string; propertyId: string } | null> {
  return run(FORM_OPERATIONS.RESOLVE_DEAL_LAUNCH_CONTEXT, { dealId }, 'forms:resolveDealLaunchContext')
}

export async function bindFormInstanceToDirectContext(input: {
  formInstanceId: string
  personId: string
  propertyId: string
}): Promise<void> {
  await run<void>(FORM_OPERATIONS.BIND_DIRECT_CONTEXT, input, 'forms:bindDirectContext')
}

export async function bindListingFormContext(input: {
  formInstanceId: string
  personId: string
  propertyId: string | null
}): Promise<void> {
  await run<void>(FORM_OPERATIONS.BIND_LISTING_CONTEXT, input, 'forms:bindListingContext')
}

export async function getFormShowingId(formInstanceId: string): Promise<string | null> {
  return run<string | null>(
    FORM_OPERATIONS.GET_SHOWING_ID,
    { formInstanceId },
    'forms:getShowingId',
  )
}

export async function bindFormInstanceToShowing(input: {
  formInstanceId: string
  showingId: string
}): Promise<void> {
  await run<void>(FORM_OPERATIONS.BIND_SHOWING, input, 'forms:bindShowing')
}

/* ------------------------------------------------------------------ *
 * Signers.
 * ------------------------------------------------------------------ */

export async function listFormSignerPeople(
  formId: string,
): Promise<FormSignerPerson[]> {
  return run<FormSignerPerson[]>(
    FORM_OPERATIONS.LIST_SIGNER_PEOPLE,
    { formInstanceId: formId },
    'forms:listSignerPeople',
  )
}
