'use server'

import { revalidatePath } from 'next/cache'

import { AuthError } from '@/lib/auth/errors'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import type { ActingUser } from '@/lib/auth/types'
import { executeCommand } from '@/lib/commands'
import { DOCUMENT_ISSUE } from '@/lib/commands/command-types'
import type { CommandOutcome } from '@/lib/workflow/contracts'
import type { SignatureRequest } from '@/lib/signature/contracts'
import { getSignatureApplication } from '@/lib/signature/runtime'
import {
  formSupportsSigning,
  isUsableSignerEmail,
} from '@/lib/forms/signer-resolution'
import {
  getIssuedDocumentForFormInstance,
  getMediaBytes,
} from '@/db/issued-document'
import { getActiveSignatureRequestForDocument } from '@/db/signature-request'
import { getTemplate } from '@/lib/forms/template-registry'
import { applyGrokFields, requestGrokFormFill } from '@/lib/forms/grok-fill'
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

export async function grokFillFormAction(input: {
  templateId: string
  prompt: string
  fieldValues: Record<string, string>
  detailsText: string
}): Promise<
  FormActionResult<{
    fieldValues: Record<string, string>
    body: string | null
    note: string
  }>
> {
  return authorizedFormWrite(async () => {
    const prompt = input.prompt.trim()
    if (!prompt) return fail('validation', 'Tell Grok what happened first.')
    const template = getTemplate(input.templateId)
    if (!template) return fail('not-found', 'Template not found.')
    try {
      const filled = await requestGrokFormFill({
        prompt,
        template,
        fieldValues: input.fieldValues,
        detailsText: input.detailsText,
      })
      return ok({
        fieldValues: applyGrokFields(template, input.fieldValues, filled.fieldValues),
        body: filled.body,
        note: filled.note,
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : ''
      if (message.includes('not configured')) {
        return fail(
          'unknown',
          'Grok is not connected on this server yet. Add the xAI API key and restart.',
        )
      }
      return fail(
        'unknown',
        'Grok could not fill the form right now. Try again in a moment.',
      )
    }
  })
}

export type FormSignatureSendData = {
  signatureRequestId: string
  documentId: string
  issuedVersion: number
  status: string
  existing: boolean
  signerName: string
  signerEmail: string
}

export async function sendFormForSignatureAction(
  formId: string,
  input: {
    signerName: string
    signerEmail: string
    fieldValues: Record<string, string>
    sections: Record<string, string>
  },
): Promise<FormActionResult<FormSignatureSendData>> {
  return authorizedFormWrite(async (actor) => {
    if (!formId.trim()) return fail('validation', 'formId is required.')
    const signerName = input.signerName.trim()
    const signerEmail = input.signerEmail.trim()
    if (!signerName) return fail('validation', 'Signer name is required.')
    if (!signerEmail) {
      return fail('validation', 'Signer email is required.')
    }
    if (!isUsableSignerEmail(signerEmail)) {
      return fail(
        'validation',
        'Please enter a valid signer email and try again.',
      )
    }

    const form = await getFormInstance(formId)
    if (!form) return fail('not-found', 'Form instance not found.')
    const template = getTemplate(form.templateId)
    if (!template) return fail('not-found', 'Template not found.')
    if (!formSupportsSigning(template)) {
      return fail('validation', 'This form is not set up for signature.')
    }

    const draftChanged =
      JSON.stringify(form.fieldValues) !== JSON.stringify(input.fieldValues) ||
      JSON.stringify(form.sections) !== JSON.stringify(input.sections)
    if (draftChanged) {
      const updated = await updateFormInstance(formId, {
        fieldValues: input.fieldValues,
        sections: input.sections,
      })
      if (!updated) return fail('not-found', 'Form instance not found.')
    }

    let issued = await getIssuedDocumentForFormInstance(formId)
    const active = issued
      ? await getActiveSignatureRequestForDocument(issued.documentId)
      : null
    const formNewerThanIssued =
      Boolean(issued?.createdAt) &&
      new Date(form.updatedAt).getTime() >
        new Date(issued?.createdAt ?? 0).getTime() + 2000

    if (active && !draftChanged) {
      return ok({
        signatureRequestId: active.id,
        documentId: issued!.documentId,
        issuedVersion: issued!.issuedVersion,
        status: active.status,
        existing: true,
        signerName,
        signerEmail,
      })
    }

    if (!issued || draftChanged || (!active && formNewerThanIssued)) {
      const issueResult = await executeCommand({
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
      if (issueResult.outcome !== 'success' || !issueResult.value) {
        const raw = issueResult.message ?? 'Could not save the PDF before sending.'
        return fail(
          outcomeCode(issueResult.outcome),
          raw.replace(/^document\.issue failed:\s*/i, ''),
        )
      }
      const value = issueResult.value as {
        documentId: string
        issuedVersion: number
        checksum: string
      }
      issued = {
        documentId: value.documentId,
        issuedVersion: value.issuedVersion,
        checksum: value.checksum,
        createdAt: new Date().toISOString(),
        mediaId: null,
      }
    }

    const media = issued.mediaId
      ? await getMediaBytes(issued.mediaId)
      : await getMediaBytesForDocument(issued.documentId)
    if (!media || media.bytes.length === 0) {
      return fail(
        'validation',
        'The issued PDF is missing. Save the document and try again.',
      )
    }

    let sendResult
    try {
      sendResult = await getSignatureApplication().send(
        {
          transactionDocumentId: issued.documentId,
          recipients: [
            {
              role: 'signer',
              name: signerName,
              email: signerEmail,
              order: 1,
            },
          ],
          createdByUserId: actor.appUserId,
        },
        { actorAppUserId: actor.appUserId },
      )
    } catch (error) {
      const detail = error instanceof Error ? error.message : ''
      console.error('Signature send failed.', detail || error)
      if (detail.includes('BoldSign config is incomplete')) {
        return fail(
          'unknown',
          'Signature sending is not configured on this server. Add the BoldSign keys and restart, then try again.',
        )
      }
      return fail(
        'unknown',
        'Could not send document for signature. Please try again.',
      )
    }

    if (sendResult.outcome !== 'success') {
      return fail(
        outcomeCode(sendResult.outcome),
        sendResult.message && !sendResult.message.toLowerCase().includes('boldsign')
          ? sendResult.message
          : 'Could not send document for signature. Please try again.',
      )
    }

    const request = (
      sendResult.value as { signatureRequest?: SignatureRequest } | undefined
    )?.signatureRequest
    const status = request?.status ?? 'sent'
    if (status === 'error') {
      return fail(
        'unknown',
        'Could not send document for signature. Please try again.',
      )
    }

    revalidatePath(`/portal/forms/${formId}`)
    revalidatePath('/portal/documents')
    return ok({
      signatureRequestId: request?.id ?? String(sendResult.aggregateId ?? ''),
      documentId: issued.documentId,
      issuedVersion: issued.issuedVersion,
      status,
      existing: Boolean(active && !draftChanged),
      signerName,
      signerEmail,
    })
  })
}

async function getMediaBytesForDocument(documentId: string) {
  const { sql } = await import('@/db/client')
  const rows = await sql`
    select media_id from transaction_document where id = ${documentId} limit 1
  `
  const mediaId = rows[0]?.media_id ? String(rows[0].media_id) : null
  if (!mediaId) return null
  return getMediaBytes(mediaId)
}
