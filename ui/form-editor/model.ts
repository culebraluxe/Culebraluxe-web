import type { PageIntentMap } from '../runtime'

export type FormEditorIssuedDocument = {
  documentId: string
  issuedVersion: number
  checksum: string
  contentFingerprint: string | null
}

export type FormEditorSignatureState = {
  id: string
  status: string
}

export type FormEditorSavedSnapshot = {
  values: Record<string, string>
  sections: Record<string, string>
  detailsText: string
}

export type FormEditorPageModel = {
  values: Record<string, string>
  sections: Record<string, string>
  detailsText: string
  bodyEdited: boolean
  saved: FormEditorSavedSnapshot
  message: string | null
  error: string | null
  issued: FormEditorIssuedDocument | null
  signatureState: FormEditorSignatureState | null
  busy: boolean
  draftSaving: boolean
}

export function isFormEditorDirty(model: Readonly<FormEditorPageModel>): boolean {
  return (
    JSON.stringify(model.values) !== JSON.stringify(model.saved.values) ||
    model.detailsText !== model.saved.detailsText
  )
}

export type FormEditorCreateContext = {
  templateId: string
  dealId?: string
  personId?: string
  propertyId?: string
}

export type FormEditorSendSignatureRequest = {
  signerPersonId: string | null
  signerRole: string | null
  signerName: string
  signerEmail: string
}

export type FormEditorSendSignatureResponse = {
  documentId: string
  issuedVersion: number
  signatureRequestId: string
  status: string
  signerName: string
  signerEmail: string
  signerCount: number
}

type EmptyPayload = Record<string, never>

export type FormEditorIntentMap = {
  'formEditor.fieldChanged': {
    request: { name: string; value: string }
    response: void
  }
  'formEditor.detailsChanged': {
    request: { value: string }
    response: void
  }
  'formEditor.saveDraft': {
    request: { quiet?: boolean }
    response: boolean
  }
  'formEditor.discard': {
    request: EmptyPayload
    response: void
  }
  'formEditor.create': {
    request: FormEditorCreateContext
    response: string | null
  }
  'formEditor.grokFill': {
    request: { prompt: string }
    response: string
  }
  'formEditor.issue': {
    request: EmptyPayload
    response: FormEditorIssuedDocument | null
  }
  'formEditor.sendSignature': {
    request: FormEditorSendSignatureRequest
    response: FormEditorSendSignatureResponse | null
  }
  'formEditor.feedback': {
    request: { message?: string | null; error?: string | null }
    response: void
  }
  'formEditor.busyChanged': {
    request: { busy: boolean }
    response: void
  }
} satisfies PageIntentMap
