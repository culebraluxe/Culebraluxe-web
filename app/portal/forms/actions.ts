'use server'

import { revalidatePath } from 'next/cache'

import { AuthError } from '@/lib/auth/errors'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import type { ActingUser } from '@/lib/auth/types'
import { executeCommand } from '@/lib/commands'
import { DOCUMENT_ISSUE } from '@/lib/commands/command-types'
import { SIGNATURE_REQUEST_SEND } from '@/lib/commands/command-types'
import type { CommandOutcome } from '@/lib/workflow/contracts'
import { getTemplate } from '@/lib/forms/template-registry'
import {
  emptyDealFacts,
  emptySectionValues,
  prefillFieldValues,
} from '@/lib/forms/offer-letter-data'
import {
  createFormInstance,
  getDealFormFacts,
  getFormInstance,
  seedFormParticipantsFromDeal,
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

async function authorizedFormWrite<T>(
  handler: (actor: ActingUser) => Promise<FormActionResult<T>>,
): Promise<FormActionResult<T>> {
  try {
    return await runAuthorized(
      getPortalSessionAdapter(),
      'deal.write',
      handler,
    )
  } catch (error) {
    if (error instanceof AuthError) {
      return fail('unknown', error.message)
    }
    throw error
  }
}

export async function createOfferLetterFormAction(
  dealId: string,
): Promise<FormActionResult<{ formId: string }>> {
  return createFormAction({ templateId: 'OFFER-01', dealId })
}

export async function createFormAction(input: {
  templateId: string
  dealId?: string
  personId?: string
  propertyId?: string
}): Promise<FormActionResult<{ formId: string }>> {
  return authorizedFormWrite(async (actor) => {
    const template = getTemplate(input.templateId)
    if (!template) return fail('validation', 'Template not found.')
    const dealId = input.dealId?.trim() || null
    const personId = input.personId?.trim() || null
    const propertyId = input.propertyId?.trim() || null
    if (!dealId && !personId && !propertyId) {
      return fail('validation', 'Select a deal, client, or property.')
    }
    let facts = emptyDealFacts()
    if (dealId) {
      const dealFacts = await getDealFormFacts(dealId)
      if (!dealFacts) return fail('not-found', 'Deal not found.')
      facts = dealFacts
    } else {
      const { sql } = await import('@/db/client')
      if (personId) {
        const rows = await sql`select display_name from person where id = ${personId} limit 1`
        const name = rows[0]?.display_name ? String(rows[0].display_name) : null
        facts.personDisplayName = name
        facts.clientName = name
      }
      if (propertyId) {
        const rows = await sql`select name, location from property where id = ${propertyId} limit 1`
        facts.propertyName = rows[0]?.name ? String(rows[0].name) : null
        facts.propertyLocation = rows[0]?.location ? String(rows[0].location) : null
        facts.propertyLabel = facts.propertyName
      }
    }
    try {
      const instance = await createFormInstance({
        templateId: template.id,
        templateVersion: template.version,
        dealId,
        personId,
        propertyId,
        fieldValues: prefillFieldValues(template, facts),
        sections: emptySectionValues(template),
        createdByUserId: actor.appUserId,
      })
      if (dealId) {
        await seedFormParticipantsFromDeal(instance.id, dealId)
      }
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
  return authorizedFormWrite(async () => {
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
  return authorizedFormWrite(async (actor) => {
    if (!formId.trim()) return fail('validation', 'formId is required.')
    const form = await getFormInstance(formId)
    if (!form) return fail('not-found', 'Form instance not found.')
    const result = await executeCommand({
      commandId: crypto.randomUUID(),
      commandType: DOCUMENT_ISSUE,
      actorAppUserId: actor.appUserId,
      aggregateType: 'transaction_document',
      aggregateId: form.dealId ?? form.id,
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

export async function sendIssuedFormForSignatureAction(
  documentId: string,
): Promise<FormActionResult<{ signatureRequestId: string }>> {
  return authorizedFormWrite(async (actor) => {
    if (!documentId.trim()) return fail('validation', 'documentId is required.')
    const result = await executeCommand({
      commandId: crypto.randomUUID(),
      commandType: SIGNATURE_REQUEST_SEND,
      actorAppUserId: actor.appUserId,
      aggregateType: 'signature_request',
      aggregateId: documentId,
      correlationId: null,
      causationId: null,
      requestedAt: new Date().toISOString(),
      input: {
        transactionDocumentId: documentId,
        recipients: [
          {
            role: 'signer',
            name: 'Document party',
            email: 'party@culebraluxe.com',
            order: 1,
          },
        ],
      },
    })
    if (result.outcome === 'success') {
      revalidatePath('/portal/documents')
      return ok({
        signatureRequestId: String(result.aggregateId ?? documentId),
      })
    }
    return fail(
      outcomeCode(result.outcome),
      result.message ?? 'Could not send for signature.',
    )
  })
}
