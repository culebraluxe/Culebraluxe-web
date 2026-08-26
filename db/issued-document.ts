import { createHash } from 'node:crypto'

import type { CommandResult } from '../lib/workflow/contracts'
import { getTemplate } from '../lib/forms/template-registry'
import { renderFormPdfArtifact } from '../lib/forms/pdf'
import { validateFormValues } from '../lib/forms/offer-letter-data'
import { neonTx, type TxRunner } from './tx'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from './workflow-command-receipt'
import { createTransactionDocument } from './transaction-document'
import { getFormInstance, updateFormInstance } from './document-form-instance'
import { listFormSignerPeople } from './form-signer'
import { canonicalizeExecutionParticipants } from '../lib/agreements/participants'
import type { QueryExecutor } from './query-executor'
import { resolveBrokerSignatureForIssuance } from './broker-signature'

// ---------------------------------------------------------------------------
// DOC-06 — canonical issued-document issuance service.
//
// The HARD INVARIANT lives here: FORMS ARE MUTABLE, ISSUED DOCUMENTS ARE
// IMMUTABLE. Issuance is a durable business action executed in ONE transaction:
//
//   claim receipt → load form + template → validate required values →
//   render PDF deterministically → sha256 checksum → append a NEW `media` row
//   (bytes never overwritten) → compute lineage version + supersession →
//   insert a NEW `transaction_document` row with full issued evidence →
//   mark the form instance 'issued' → finalize receipt.
//
// A second issuance from edited values creates v2; v1's row + media bytes
// remain byte-for-byte unchanged (its state flips to 'superseded').
// ---------------------------------------------------------------------------

export type IssueDocumentInput = {
  commandId: string
  formInstanceId: string
  actorAppUserId?: string | null
  /** Deterministic human issuance boundary (CommandEnvelope.requestedAt). */
  issuedAt?: string | null
}

/** Read the exact stored bytes of a media row (for download/verification). */
export async function getMediaBytes(
  mediaId: string,
  execute?: QueryExecutor,
): Promise<{ bytes: Buffer; filename: string; mimeType: string } | null> {
  const q = execute ?? (await import('./client')).sql
  const rows = await q`
    select file_data, filename, mime_type
    from media
    where id = ${mediaId}
    limit 1
  `
  const row = rows[0] as
    | { file_data: Buffer | Uint8Array | null; filename: string | null; mime_type: string | null }
    | undefined
  if (!row) return null
  return {
    bytes: Buffer.isBuffer(row.file_data)
      ? row.file_data
      : Buffer.from(row.file_data ?? []),
    filename: String(row.filename ?? 'document.pdf'),
    mimeType: String(row.mime_type ?? 'application/pdf'),
  }
}

export async function getIssuedDocumentForFormInstance(
  formInstanceId: string,
  execute?: QueryExecutor,
): Promise<{
  documentId: string
  issuedVersion: number
  checksum: string
  createdAt: string
  mediaId: string | null
  sourceSnapshot: Record<string, unknown> | null
} | null> {
  const q = execute ?? (await import('./client')).sql
  const rows = await q`
    select id, issued_version, issued_checksum_sha256, created_at, media_id,
      source_snapshot
    from transaction_document
    where form_instance_id = ${formInstanceId}
      and source = 'generated'
    order by issued_version desc nulls last, created_at desc
    limit 1
  `
  const row = rows[0] as
    | {
        id?: unknown
        issued_version?: unknown
        issued_checksum_sha256?: unknown
        created_at?: unknown
        media_id?: unknown
        source_snapshot?: unknown
      }
    | undefined
  if (!row?.id) return null
  return {
    documentId: String(row.id),
    issuedVersion: Number(row.issued_version ?? 1),
    checksum: String(row.issued_checksum_sha256 ?? ''),
    createdAt: row.created_at
      ? new Date(row.created_at as string).toISOString()
      : '',
    mediaId: row.media_id ? String(row.media_id) : null,
    sourceSnapshot:
      row.source_snapshot && typeof row.source_snapshot === 'object'
        ? (row.source_snapshot as Record<string, unknown>)
        : null,
  }
}

/** Preview must derive the same lineage version that issuance will use. */
export async function getNextIssuedVersionForTemplate(
  input: { dealId: string | null; templateId: string },
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await import('./client')).sql
  const rows = await q`
    select issued_version
    from transaction_document
    where deal_id is not distinct from ${input.dealId}
      and template_id = ${input.templateId}
      and source = 'generated'
      and issued_version is not null
    order by issued_version desc, created_at desc
    limit 1
  `
  return Number((rows[0] as { issued_version?: unknown } | undefined)?.issued_version ?? 0) + 1
}

async function insertIssuedMedia(
  tx: QueryExecutor,
  bytes: Buffer,
  filename: string,
): Promise<string> {
  const rows = await tx`
    insert into media (file_data, filename, mime_type, file_size, media_type)
    values (${bytes}, ${filename}, 'application/pdf', ${bytes.length}, 'document')
    returning id
  `
  const row = rows[0] as { id?: unknown } | undefined
  if (!row?.id) {
    throw new Error('Issued-document media insert returned no row.')
  }
  return String(row.id)
}

/** Resolve the deal's active client person (party) id, if available. */
async function resolvePartyPersonId(
  tx: QueryExecutor,
  dealId: string,
): Promise<string | null> {
  const rows = await tx`
    select person_id
    from deal_participant
    where deal_id = ${dealId}
      and role = 'client'
      and person_id is not null
      and active
    order by created_at asc, id
    limit 1
  `
  const row = rows[0] as { person_id?: unknown } | undefined
  return row?.person_id ? String(row.person_id) : null
}

// __PART2__
/**
 * Issue a form instance as an immutable canonical transaction document.
 * Claim-first receipt idempotency: the same commandId executes at most once;
 * a replayed caller observes the winner's stored result.
 */
export async function issueFormDocument(
  input: IssueDocumentInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (!input.formInstanceId.trim()) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'document.issue requires a formInstanceId.',
      replayed: false,
    }
  }

  return run(async (tx) => {
    const claimed = await claimReceipt(
      tx,
      input.commandId,
      input.actorAppUserId ?? null,
    )
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }

    const form = await getFormInstance(input.formInstanceId, tx)
    if (!form) {
      return {
        commandId: input.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: 'document.issue failed: form instance not found.',
        replayed: false,
      }
    }
    const template = getTemplate(form.templateId)
    if (!template) {
      return {
        commandId: input.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: `document.issue failed: unknown template ${form.templateId}.`,
        replayed: false,
      }
    }

    const issues = validateFormValues(template, form.fieldValues)
    if (issues.length > 0) {
      return {
        commandId: input.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: `document.issue failed: ${issues[0].message}`,
        replayed: false,
      }
    }

    // Lineage: next 1-based version within (deal, template) + supersession.
    const priorRows = await tx`
      select id, issued_version
      from transaction_document
      where deal_id is not distinct from ${form.dealId}
        and template_id = ${form.templateId}
        and source = 'generated'
        and issued_version is not null
      order by issued_version desc, created_at desc
      limit 1
    `
    const prior = priorRows[0] as
      | { id: string; issued_version: number }
      | undefined
    const issuedVersion = Number(prior?.issued_version ?? 0) + 1
    const supersedesId = prior?.id ?? null

    // PARTICIPANT CARDINALITY (CRM-27): resolve the immutable participant set
    // before rendering so each printed signature block and provider-neutral
    // anchor belongs to the exact issued slot it can satisfy.
    const issuedSlots = canonicalizeExecutionParticipants(
      await listFormSignerPeople(form.id, tx),
    )

    const brokerSignature = await resolveBrokerSignatureForIssuance(
      {
        template,
        values: form.fieldValues,
        participants: issuedSlots,
        actorAppUserId: input.actorAppUserId ?? null,
        issuedAt: input.issuedAt ?? null,
      },
      tx,
    )
    if (!brokerSignature.ok) {
      await finalizeReceipt(
        tx,
        input.commandId,
        brokerSignature.outcome,
        null,
        brokerSignature.message,
        input.actorAppUserId ?? null,
      )
      return {
        commandId: input.commandId,
        outcome: brokerSignature.outcome,
        emittedEvents: [],
        aggregateId: null,
        message: brokerSignature.message,
        replayed: false,
      }
    }

    // Deterministic composition + resolved geometry BEFORE any irreversible
    // write. Preview consumes this same renderer; the bytes and signature
    // anchors are one artifact, not independently inferred representations.
    const rendered = await renderFormPdfArtifact(
      template,
      form.fieldValues,
      form.sections,
      issuedVersion,
      {
        participants: issuedSlots,
        appliedSignatures: brokerSignature.signatures,
      },
    )
    const pdfBytes = rendered.bytes
    const checksum = createHash('sha256').update(pdfBytes).digest('hex')

    const mediaId = await insertIssuedMedia(
      tx,
      pdfBytes,
      `${template.id.toLowerCase()}-v${issuedVersion}.pdf`,
    )

    if (supersedesId) {
      await tx`
        update transaction_document
        set state = 'superseded', updated_at = now()
        where id = ${supersedesId}
          and state <> 'superseded'
      `
    }

    const partyPersonId =
      form.personId ??
      (form.dealId ? await resolvePartyPersonId(tx, form.dealId) : null)

    const document = await createTransactionDocument(
      {
        dealId: form.dealId,
        documentType: 'agreement',
        documentTypeLabel: template.documentTypeLabel,
        title: `${template.displayName} v${issuedVersion}`,
        state: 'ready',
        source: 'generated',
        preparedByUserId: input.actorAppUserId ?? null,
        partyPersonId,
        mediaId,
        supersedesDocumentId: supersedesId,
        issuedChecksumSha256: checksum,
        templateId: template.id,
        templateVersion: template.version,
        sourceSnapshot: {
          templateId: template.id,
          templateVersion: template.version,
          fieldValues: form.fieldValues,
          sections: form.sections,
          issuedParticipants: issuedSlots,
          signatureAnchors: rendered.signatureAnchors,
          appliedSignatures: rendered.appliedSignatures,
          pdfLayout: {
            pageCount: rendered.pageCount,
            pageSize: rendered.pageSize,
            coordinateSpace: 'pdf-points-bottom-left',
          },
        },
        issuedVersion,
        formInstanceId: form.id,
      },
      tx,
    )

    await updateFormInstance(form.id, { status: 'issued' }, tx)

    await finalizeReceipt(
      tx,
      input.commandId,
      'success',
      document.id,
      null,
      input.actorAppUserId ?? null,
    )

    return {
      commandId: input.commandId,
      outcome: 'success',
      emittedEvents: [],
      aggregateId: document.id,
      message: null,
      replayed: false,
      value: {
        documentId: document.id,
        mediaId,
        issuedVersion,
        checksum,
        issuedAt: document.createdAt,
      },
    }
  })
}
