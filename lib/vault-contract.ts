import type { CommandResult } from '@/lib/workflow/contracts'

export type TransactionDocumentType =
  | 'agreement'
  | 'addendum'
  | 'disclosure'
  | 'title'
  | 'financing'
  | 'inspection'
  | 'appraisal'
  | 'closing'
  | 'other'

export type TransactionDocumentState =
  | 'draft'
  | 'ready'
  | 'sent'
  | 'signed'
  | 'voided'
  | 'superseded'

export type TransactionDocumentSource =
  | 'upload'
  | 'generated'
  | 'imported'
  | 'provider'

export type IssuedDocumentListItem = {
  id: string
  dealId: string
  propertyId: string | null
  documentTypeLabel: string | null
  title: string | null
  state: TransactionDocumentState
  templateId: string | null
  templateVersion: number | null
  issuedVersion: number | null
  issuedChecksumSha256: string | null
  issuedByDisplayName: string | null
  partyName: string | null
  propertyName: string | null
  dealName: string | null
  createdAt: string
  signedArtifactAvailable: boolean
  signedAuditAvailable: boolean
}

export type TransactionDocument = {
  id: string
  dealId: string | null
  documentType: TransactionDocumentType
  documentTypeLabel: string | null
  title: string | null
  state: TransactionDocumentState
  source: TransactionDocumentSource
  sourceSystem: string | null
  sourceExternalId: string | null
  preparedByUserId: string | null
  partyPersonId: string | null
  mediaId: string | null
  signedMediaId: string | null
  signedAuditMediaId: string | null
  signedAt: string | null
  supersedesDocumentId: string | null
  issuedChecksumSha256: string | null
  templateId: string | null
  templateVersion: number | null
  sourceSnapshot: Record<string, unknown> | null
  issuedVersion: number | null
  formInstanceId: string | null
  createdAt: string
  updatedAt: string
}

export type RustVaultTransactionDocument = {
  id: string
  dealId: string | null
  documentType: TransactionDocumentType
  documentTypeLabel: string | null
  title: string | null
  state: TransactionDocumentState
  source: TransactionDocumentSource
  sourceSystem: string | null
  sourceExternalId: string | null
  preparedByUserId: string | null
  partyPersonId: string | null
  mediaId: string | null
  signedArtifact: { mediaId: string; signedAt: string } | null
  signedAuditMediaId: string | null
  supersedesDocumentId: string | null
  issuedEvidence: {
    checksumSha256: string
    templateId: string
    templateVersion: number
    sourceSnapshot: Record<string, unknown> | null
    issuedVersion: number
    formInstanceId: string
  } | null
  createdAt: string
  updatedAt: string
}

export function mapRustVaultTransactionDocument(
  value: RustVaultTransactionDocument,
): TransactionDocument {
  return {
    id: value.id,
    dealId: value.dealId,
    documentType: value.documentType,
    documentTypeLabel: value.documentTypeLabel,
    title: value.title,
    state: value.state,
    source: value.source,
    sourceSystem: value.sourceSystem,
    sourceExternalId: value.sourceExternalId,
    preparedByUserId: value.preparedByUserId,
    partyPersonId: value.partyPersonId,
    mediaId: value.mediaId,
    signedMediaId: value.signedArtifact?.mediaId ?? null,
    signedAuditMediaId: value.signedAuditMediaId,
    signedAt: value.signedArtifact?.signedAt ?? null,
    supersedesDocumentId: value.supersedesDocumentId,
    issuedChecksumSha256: value.issuedEvidence?.checksumSha256 ?? null,
    templateId: value.issuedEvidence?.templateId ?? null,
    templateVersion: value.issuedEvidence?.templateVersion ?? null,
    sourceSnapshot: value.issuedEvidence?.sourceSnapshot ?? null,
    issuedVersion: value.issuedEvidence?.issuedVersion ?? null,
    formInstanceId: value.issuedEvidence?.formInstanceId ?? null,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  }
}

export type IssueDocumentInput = {
  commandId: string
  formInstanceId: string
  actorAppUserId?: string | null
  issuedAt?: string | null
}

export type IssuedDocumentForFormInstance = {
  documentId: string
  issuedVersion: number
  checksum: string
  createdAt: string
  mediaId: string | null
  sourceSnapshot: Record<string, unknown> | null
}

export type VaultMediaBytes = {
  bytes: Buffer
  filename: string
  mimeType: string
}

export type ContractIssuedLineage = {
  id: string
  issuedVersion: number
}

export type VaultCommandResult = CommandResult
