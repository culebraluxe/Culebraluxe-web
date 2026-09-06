import type { QueryExecutor } from './query-executor'

// Canonical Contract/document seam.
//
// The legacy transaction-document repository still accepts Deal/Person context
// because a few real Listing agreements depend on it. New Contract-owned form
// issuance comes through this module instead: Contract identity is explicit,
// never inferred from a Person, Property, Deal, template, or latest row.

async function executor(): Promise<QueryExecutor> {
  return (await import('./client')).sql
}

export async function getFormContractId(
  formInstanceId: string,
  execute?: QueryExecutor,
): Promise<string | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select contract_id
    from document_form_instance
    where id = ${formInstanceId}
    limit 1
  `
  const row = rows[0] as { contract_id?: unknown } | undefined
  return row?.contract_id ? String(row.contract_id) : null
}

export async function bindFormInstanceToContract(
  input: { formInstanceId: string; contractId: string },
  execute?: QueryExecutor,
): Promise<void> {
  const formInstanceId = input.formInstanceId.trim()
  const contractId = input.contractId.trim()
  if (!formInstanceId || !contractId) {
    throw new Error('bindFormInstanceToContract requires formInstanceId and contractId.')
  }

  const q = execute ?? (await executor())
  const rows = await q`
    update document_form_instance
    set contract_id = ${contractId}, updated_at = now()
    where id = ${formInstanceId}
      and (contract_id is null or contract_id = ${contractId})
    returning id
  `
  if (!rows[0]) {
    throw new Error(
      'Form instance was not found or is already bound to a different Contract.',
    )
  }
}

export type ContractIssuedLineage = {
  id: string
  issuedVersion: number
}

export async function getPriorContractIssuedDocument(
  input: { contractId: string; templateId: string },
  execute?: QueryExecutor,
): Promise<ContractIssuedLineage | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, issued_version
    from transaction_document
    where contract_id = ${input.contractId}
      and template_id = ${input.templateId}
      and source = 'generated'
      and issued_version is not null
    order by issued_version desc, created_at desc
    limit 1
  `
  const row = rows[0] as { id?: unknown; issued_version?: unknown } | undefined
  if (!row?.id) return null
  return {
    id: String(row.id),
    issuedVersion: Number(row.issued_version ?? 0),
  }
}

export type CreateContractIssuedEvidenceInput = {
  contractId: string
  documentTypeLabel: string
  title: string
  preparedByUserId?: string | null
  mediaId: string
  supersedesDocumentId?: string | null
  issuedChecksumSha256: string
  templateId: string
  templateVersion: number
  sourceSnapshot: Record<string, unknown>
  issuedVersion: number
  formInstanceId: string
}

export async function createContractIssuedEvidence(
  input: CreateContractIssuedEvidenceInput,
  execute?: QueryExecutor,
): Promise<{ id: string; createdAt: string }> {
  const contractId = input.contractId.trim()
  if (!contractId) {
    throw new Error('createContractIssuedEvidence requires an explicit contractId.')
  }
  if (!Number.isInteger(input.issuedVersion) || input.issuedVersion < 1) {
    throw new Error('createContractIssuedEvidence requires issuedVersion >= 1.')
  }

  const q = execute ?? (await executor())
  const rows = await q`
    insert into transaction_document (
      contract_id, deal_id, document_type, document_type_label, title, state,
      source, prepared_by_user_id, party_person_id, media_id,
      supersedes_document_id, issued_checksum_sha256, template_id,
      template_version, source_snapshot, issued_version, form_instance_id
    ) values (
      ${contractId}, null, 'agreement', ${input.documentTypeLabel}, ${input.title},
      'ready', 'generated', ${input.preparedByUserId ?? null}, null, ${input.mediaId},
      ${input.supersedesDocumentId ?? null}, ${input.issuedChecksumSha256},
      ${input.templateId}, ${input.templateVersion},
      ${JSON.stringify(input.sourceSnapshot)}::jsonb, ${input.issuedVersion},
      ${input.formInstanceId}
    )
    returning id, created_at
  `
  const row = rows[0] as { id?: unknown; created_at?: unknown } | undefined
  if (!row?.id) {
    throw new Error('Contract issued-document insert returned no row.')
  }
  return {
    id: String(row.id),
    createdAt: row.created_at
      ? new Date(row.created_at as string).toISOString()
      : '',
  }
}
