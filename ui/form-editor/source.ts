'use client'

import {
  createFormAction,
  grokFillFormAction,
  issueFormAction,
  sendFormForSignatureAction,
  updateFormAction,
} from '@/app/portal/forms/actions'
import type { ListingCanonicalSnapshot } from '@/lib/forms/listing-field-binding'
import type {
  FormEditorCreateContext,
  FormEditorSendSignatureRequest,
  FormEditorSendSignatureResponse,
} from './model'

export type FormEditorSourceResult<T> =
  | { ok: true; data: T }
  | { ok: false; message: string }

export type FormEditorUpdateResult = {
  updated: boolean
  canonicalUpdates?: string[]
}

export type FormEditorGrokResult = {
  fieldValues: Record<string, string>
  body: string | null
  note: string
}

export type FormEditorIssueResult = {
  documentId: string
  issuedVersion: number
  checksum: string
}

export interface FormEditorSource {
  create(input: FormEditorCreateContext): Promise<FormEditorSourceResult<{ formId: string }>>
  update(
    formId: string,
    fieldValues: Record<string, string>,
    sections: Record<string, string>,
  ): Promise<FormEditorSourceResult<FormEditorUpdateResult>>
  refreshListing(personId: string): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>>
  selectListingClient(
    formId: string,
    personId: string,
  ): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>>
  grokFill(input: {
    formId: string
    prompt: string
    fieldValues: Record<string, string>
    detailsText: string
  }): Promise<FormEditorSourceResult<FormEditorGrokResult>>
  issue(formId: string): Promise<FormEditorSourceResult<FormEditorIssueResult>>
  sendSignature(
    formId: string,
    input: FormEditorSendSignatureRequest & {
      fieldValues: Record<string, string>
      sections: Record<string, string>
    },
  ): Promise<FormEditorSourceResult<FormEditorSendSignatureResponse>>
}

function normalizeFailure(result: { message?: string }, fallback: string) {
  return { ok: false as const, message: result.message ?? fallback }
}

export class ActionFormEditorSource implements FormEditorSource {
  async create(input: FormEditorCreateContext): Promise<FormEditorSourceResult<{ formId: string }>> {
    const result = await createFormAction(input)
    return result.ok ? result : normalizeFailure(result, 'Could not start a new form.')
  }

  async update(
    formId: string,
    fieldValues: Record<string, string>,
    sections: Record<string, string>,
  ): Promise<FormEditorSourceResult<FormEditorUpdateResult>> {
    const result = await updateFormAction(formId, fieldValues, sections)
    return result.ok ? result : normalizeFailure(result, 'Could not save.')
  }

  async refreshListing(personId: string): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>> {
    try {
      const response = await fetch(
        `/api/portal/form-sidecar/listing?personId=${encodeURIComponent(personId)}`,
        { cache: 'no-store' },
      )
      const body = (await response.json()) as ListingCanonicalSnapshot & { error?: string }
      if (!response.ok) {
        return { ok: false, message: body.error ?? 'Could not refresh client data.' }
      }
      return { ok: true, data: body }
    } catch (error) {
      return {
        ok: false,
        message: error instanceof Error ? error.message : 'Could not refresh client data.',
      }
    }
  }

  async selectListingClient(
    formId: string,
    personId: string,
  ): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>> {
    try {
      const response = await fetch('/api/portal/form-sidecar/listing/select-client', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ formId, personId }),
      })
      const body = (await response.json()) as ListingCanonicalSnapshot & { error?: string }
      if (!response.ok) {
        return { ok: false, message: body.error ?? 'Could not select the client.' }
      }
      return { ok: true, data: body }
    } catch (error) {
      return {
        ok: false,
        message: error instanceof Error ? error.message : 'Could not select the client.',
      }
    }
  }

  async grokFill(input: {
    formId: string
    prompt: string
    fieldValues: Record<string, string>
    detailsText: string
  }): Promise<FormEditorSourceResult<FormEditorGrokResult>> {
    const result = await grokFillFormAction(input)
    return result.ok ? result : normalizeFailure(result, 'Grok could not fill the form.')
  }

  async issue(formId: string): Promise<FormEditorSourceResult<FormEditorIssueResult>> {
    const result = await issueFormAction(formId)
    return result.ok ? result : normalizeFailure(result, 'Could not save the PDF to the vault.')
  }

  async sendSignature(
    formId: string,
    input: FormEditorSendSignatureRequest & {
      fieldValues: Record<string, string>
      sections: Record<string, string>
    },
  ): Promise<FormEditorSourceResult<FormEditorSendSignatureResponse>> {
    const result = await sendFormForSignatureAction(formId, input)
    return result.ok
      ? { ok: true, data: result.data }
      : normalizeFailure(result, 'Could not send document for signature. Please try again.')
  }
}

export class InMemoryFormEditorSource implements FormEditorSource {
  readonly updates: Array<{
    formId: string
    fieldValues: Record<string, string>
    sections: Record<string, string>
  }> = []

  constructor(
    private readonly options: {
      nextFormId?: string
      listingSnapshot?: ListingCanonicalSnapshot
      canonicalUpdates?: string[]
      grok?: FormEditorGrokResult
      issued?: FormEditorIssueResult
      signature?: FormEditorSendSignatureResponse
    } = {},
  ) {}

  async create(): Promise<FormEditorSourceResult<{ formId: string }>> {
    return { ok: true, data: { formId: this.options.nextFormId ?? 'form-next' } }
  }

  async update(
    formId: string,
    fieldValues: Record<string, string>,
    sections: Record<string, string>,
  ): Promise<FormEditorSourceResult<FormEditorUpdateResult>> {
    this.updates.push({
      formId,
      fieldValues: { ...fieldValues },
      sections: { ...sections },
    })
    return {
      ok: true,
      data: {
        updated: true,
        canonicalUpdates: this.options.canonicalUpdates ?? [],
      },
    }
  }

  async refreshListing(personId: string): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>> {
    if (this.options.listingSnapshot) return { ok: true, data: this.options.listingSnapshot }
    return { ok: true, data: defaultListingSnapshot(personId) }
  }

  async selectListingClient(
    _formId: string,
    personId: string,
  ): Promise<FormEditorSourceResult<ListingCanonicalSnapshot>> {
    if (this.options.listingSnapshot) return { ok: true, data: this.options.listingSnapshot }
    return { ok: true, data: defaultListingSnapshot(personId) }
  }

  async grokFill(input: {
    fieldValues: Record<string, string>
  }): Promise<FormEditorSourceResult<FormEditorGrokResult>> {
    return {
      ok: true,
      data: this.options.grok ?? {
        fieldValues: { ...input.fieldValues },
        body: null,
        note: 'Grok complete',
      },
    }
  }

  async issue(): Promise<FormEditorSourceResult<FormEditorIssueResult>> {
    return {
      ok: true,
      data: this.options.issued ?? {
        documentId: 'doc-1',
        issuedVersion: 1,
        checksum: 'checksum',
      },
    }
  }

  async sendSignature(): Promise<FormEditorSourceResult<FormEditorSendSignatureResponse>> {
    return {
      ok: true,
      data: this.options.signature ?? {
        documentId: 'doc-1',
        issuedVersion: 1,
        signatureRequestId: 'sig-1',
        status: 'sent',
        signerName: 'Signer',
        signerEmail: 'signer@example.com',
        signerCount: 1,
      },
    }
  }
}

function defaultListingSnapshot(personId: string): ListingCanonicalSnapshot {
  return {
    personId,
    personDisplayName: 'Client',
    formInstanceId: null,
    formUpdatedAt: null,
    legalAddressPropertyId: null,
    physicalPropertyId: null,
    fields: {
      sellerName: 'Client',
      sellerResidenceAddress: '',
      property: '',
      propertyLocation: '',
      legalOwnerName: '',
      catastroNumber: '',
    },
    origins: {
      sellerName: 'person',
      sellerResidenceAddress: 'empty',
      property: 'empty',
      propertyLocation: 'empty',
      legalOwnerName: 'empty',
      catastroNumber: 'empty',
    },
  }
}
