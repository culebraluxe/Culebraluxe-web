'use server'

import { revalidatePath } from 'next/cache'

import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import { executeCommand } from '@/lib/commands'
import { DOCUMENT_ISSUE } from '@/lib/commands/command-types'
import type { CommandOutcome } from '@/lib/workflow/contracts'
import { getTemplate, OFFER_LETTER_TEMPLATE_ID } from '@/lib/forms/template-registry'
import {
  emptySectionValues,
  prefillFieldValues,
} from '@/lib/forms/offer-letter-data'
import {
  createFormInstance,
  getDealFormFacts,
  getFormInstance,
  updateFormInstance,
} from '@/db/document-form-instance'
import { PortalWriteError } from '@/lib/portal-write-error'

// ---------------------------------------------------------------------------
// DOC-07 / DOC-06 — NEXUS Forms server actions.
//
// Draft editing (create/update a form instance) is MUTABLE working state and
// intentionally does NOT go through the Business Command layer — no durable
// business record is created. Issuance IS a meaningful business action and
// routes through the canonical document.issue command (claim-first receipt,
// one transaction, immutable transaction_document + media bytes).
// ---------------------------------------------------------------------------

export type FormActionResult<T> =
  | { ok: true; data: T }
  | { ok: false; code: 'validation' | 'conflict' | 'not-found' | 'unknown'; message: string }

function ok<T>(data: T): FormActionResult<T> {
  return { ok: true, data }
}

function fail<T>(
  code: 'validation' | 'conflict' | 'not-found' | 'unknown',
  message: string,
): FormActionResult<T> {
  return { ok: false, code, message }
}

function errorCode(e: unknown): 'validation' | 'conflict' | 'not-found' | 'unknown' {
  if (e instanceof PortalWriteError) {
    if (e.code === 'validation') return 'validation'
    if (e.code === 'conflict') return 'conflict'
    if (e.code === 'not-found') return 'not-found'
  }
  console.error('Forms action failed.', e instanceof Error ? e.message : e)
  return 'unknown'
}

function outcomeCode(outcome: CommandOutcome): 'validation' | 'conflict' | 'not-found' | 'unknown' {
  if (outcome === 'validation_failure') return 'validation'
  if (outcome === 'conflict') return 'conflict'
  if (outcome === 'not_found') return 'not-found'
  return 'unknown'
}

export async function createOfferLetterFormAction(
  dealId: string,
): Promise<FormActionResult<{ formId: string }>> {
  return runAuthorized(getPortalSessionAdapter(), 'deal.write', async (actor) => {
    if (!dealId.trim()) return fail('validation', 'A deal must be selected.')
    const template = getTemplate(OFFER_LETTER_TEMPLATE_ID)
    if (!template) return fail('validation', 'Offer Letter template not found.')
    const facts = await getDealFormFacts(dealId)
    if (!facts) return fail('not-found', 'Deal not found.')
    try {
      const instance = await createFormInstance({
        templateId: template.id,
        templateVersion: template.version,
        dealId,
        fieldValues: prefillFieldValues(template, facts),
        sections: emptySectionValues(template),
        createdByUserId: actor.appUserId,
      })
      revalidatePath('/portal/forms')
      return ok({ formId: instance.id })
    } catch (e) {
      return fail(errorCode(e), 'Could not create the form instance.')
    }
  })
}

export async function updateFormAction(
  formId: string,
  fieldValues: Record<string, string>,
  sections: Record<string, string>,
): Promise<FormActionResult<{ updated: boolean }>> {
  return runAuthorized(getPortalSessionAdapter(), 'deal.write', async () => {
    if (!formId.trim()) return fail('validation', 'formId is required.')
    try {
      const updated = await updateFormInstance(formId, { fieldValues, sections })
      if (!updated) return fail('not-found', 'Form instance not found.')
      revalidatePath(`/portal/forms/${formId}`)
      return ok({ updated: true })
    } catch (e) {
      return fail(errorCode(e), 'Could not save the form.')
    }
  })
}

export async function issueFormAction(
  formId: string,
): Promise<FormActionResult<{ documentId: string; issuedVersion: number; checksum: string }>> {
  return runAuthorized(getPortalSessionAdapter(), 'deal.write', async (actor) => {
    if (!formId.trim()) return fail('validation', 'formId is required.')
    const form = await getFormInstance(formId)
    if (!form) return fail('not-found', 'Form instance not found.')
    const result = await executeCommand({
      commandId: crypto.randomUUID(),
      commandType: DOCUMENT_ISSUE,
      actorAppUserId: actor.appUserId,
      aggregateType: 'transaction_document',
      aggregateId: form.dealId,
      correlationId: null,
      causationId: null,
      requestedAt: new Date().toISOString(),
      input: { formInstanceId: formId },
    })
    if (result.outcome === 'success' && result.value) {
      const value = result.value as {
        documentId: string
        issuedVersion: number
        checksum: string
      }
      revalidatePath('/portal/forms')
      revalidatePath('/portal/documents')
      return ok({
        documentId: value.documentId,
        issuedVersion: value.issuedVersion,
        checksum: value.checksum,
      })
    }
    return fail(
      outcomeCode(result.outcome),
      result.message ?? 'Could not issue the document.',
    )
  })
}
