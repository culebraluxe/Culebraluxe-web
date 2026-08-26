import { getTemplate } from '../forms/template-registry'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import {
  claimAgreementExecution,
  getCompletedExecutionSlots,
} from '../../db/agreement-execution'
import {
  evaluateAgreementExecution,
  isExecutionEligibleTemplate,
  resolveRequiredSlots,
  type AgreementExecutionVerdict,
} from './execution'
import { parseIssuedParticipants } from './participants'
import { parseAppliedSignatureSlotIds } from '../forms/applied-signature'

// ---------------------------------------------------------------------------
// CRM-27 — Agreement Completion evaluation (provider-neutral wiring).
//
// Composes the neutral evidence model with the required-role policy seam and
// the Agreement Execution Predicate, then records the exactly-once marker.
//
//   document → template (declared signature roles) → required roles (policy)
//   + completed signature-role evidence
//   → predicate verdict
//   → fully executed ? claim marker; shouldEmit exactly once : no-op
//
// `shouldEmit: true` tells the caller to emit the neutral AGREEMENT_FULLY_EXECUTED
// fact for this version; duplicate/replayed completions return shouldEmit: false
// (idempotent). This service never writes workflow state and never touches a
// provider.
//
// DURABILITY REPAIR (CRM-27): the whole evaluation runs through the caller-supplied
// TxRunner (`run`) — for the canonical command path this is the dispatcher's
// transaction (`ctx.run`), so the marker insert (`claimAgreementExecution`)
// commits atomically with the command receipt and the outbox row. It never
// opens a nested independent `neonTx`. A rolled-back command transaction
// leaves neither marker nor event.
//
// EVENT-ID EQUALITY (CRM-27): the canonical AGREEMENT_FULLY_EXECUTED event id is
// generated ONCE by the caller and passed in as `eventId`. It is written to
// agreement_execution.event_id (this marker), carried on the DomainEvent.eventId
// and used as outbox_message.id — so the marker and the outbox row are directly
// auditable by id (marker.event_id == outbox.id).
//
// TRUTHFUL OUTCOMES (CRM-27): a missing document is `not_found`; malformed
// identity/lineage is `validation_failure`; a document that is not an
// execution-eligible agreement (unknown template, non-PR-PNS, no Deal linkage,
// zero required participants) is `precondition_failure`. These are returned
// truthfully — never fabricated as "not fully executed" — so the command does
// not finalize a success receipt for a missing or invalid document.
// ---------------------------------------------------------------------------

/** A not-fully-executed verdict for rejection paths (no evidence was read). */
const INCOMPLETE: AgreementExecutionVerdict = {
  fullyExecuted: false,
  missingRoles: [],
  missingSlotIds: [],
  reason: 'missing_required_roles',
}

export type AgreementCompletionOutcome =
  | 'success'
  | 'not_found'
  | 'validation_failure'
  | 'precondition_failure'
  | 'conflict'
  | 'unauthorized'

export type AgreementCompletionResult = {
  /** Truthful canonical outcome (never fabricates "not fully executed"). */
  outcome: AgreementCompletionOutcome
  /** Human-readable rejection detail, when outcome is not success. */
  error: string | null
  verdict: AgreementExecutionVerdict
  /** True ONLY on the first time this document version is judged fully executed. */
  shouldEmit: boolean
  document: { documentId: string; issuedVersion: number } | null
  /** Template identity of the immutable issued document (for the event payload). */
  templateId: string | null
  /** Deal linkage of the immutable issued document (required for execution). */
  dealId: string | null
  /**
   * The canonical AGREEMENT_FULLY_EXECUTED event id (== agreement_execution.event_id
   * == outbox_message.id) when shouldEmit; null otherwise.
   */
  eventId: string | null
}

export type AgreementCompletionDeps = {
  execute: QueryExecutor
  /** REQUIRED — the interactive transaction the marker participates in. */
  run: TxRunner
  now?: () => Date
}


/**
 * Validate that a document is a workflow-integrated execution-eligible agreement
 * and resolve its immutable identity/lineage. Shared by the automatic claim and
 * the audited manual/external execution command. Returns truthful outcomes:
 * not_found / validation_failure / precondition_failure / success.
 */
export type AgreementDocumentContext = {
  outcome: 'success' | AgreementCompletionOutcome
  error: string | null
  document: { documentId: string; issuedVersion: number } | null
  templateId: string | null
  dealId: string | null
  /** The resolved template, when valid (for slot/role resolution). */
  template: ReturnType<typeof getTemplate>
  /** The immutable issued source_snapshot (participant slots), when resolved. */
  sourceSnapshot?: Record<string, unknown> | null
}

export async function resolveAgreementDocument(
  documentId: string,
  execute: QueryExecutor,
): Promise<AgreementDocumentContext> {
  const rows = await execute`
    select id, template_id, issued_version, deal_id, document_type, source_snapshot
    from transaction_document
    where id = ${documentId}
    limit 1
  `
  const row = rows[0] as
    | {
        id?: unknown
        template_id?: unknown
        issued_version?: unknown
        deal_id?: unknown
        document_type?: unknown
        source_snapshot?: unknown
      }
    | undefined
  if (!row?.id) {
    return {
      outcome: 'not_found',
      error: `Transaction document not found: ${documentId}.`,
      document: null,
      templateId: null,
      dealId: null,
      template: null,
    }
  }
  const templateId = typeof row.template_id === 'string' ? row.template_id : ''
  const issuedVersion = Number(row.issued_version ?? 0)
  const dealId = typeof row.deal_id === 'string' ? row.deal_id : null
  const documentType = typeof row.document_type === 'string' ? row.document_type : ''
  const document = { documentId, issuedVersion }

  if (!Number.isInteger(issuedVersion) || issuedVersion < 1) {
    return {
      outcome: 'validation_failure',
      error: `Invalid issued_version for ${documentId}: ${issuedVersion}.`,
      document: { documentId, issuedVersion: 0 },
      templateId: templateId || null,
      dealId,
      template: null,
    }
  }
  const template = getTemplate(templateId)
  if (!template) {
    return {
      outcome: 'validation_failure',
      error: `Unknown template ${templateId || '(none)'} for document ${documentId}.`,
      document,
      templateId: templateId || null,
      dealId,
      template: null,
    }
  }
  if (!isExecutionEligibleTemplate(templateId)) {
    return {
      outcome: 'precondition_failure',
      error: `Template ${templateId} is not execution-eligible for agreement execution.`,
      document,
      templateId,
      dealId,
      template,
    }
  }
  if (documentType !== 'agreement') {
    return {
      outcome: 'precondition_failure',
      error: `Document ${documentId} is not an agreement (type '${documentType || 'none'}').`,
      document,
      templateId,
      dealId,
      template,
    }
  }
  if (!dealId) {
    return {
      outcome: 'precondition_failure',
      error: `Document ${documentId} has no Deal linkage required for workflow-integrated execution.`,
      document,
      templateId,
      dealId,
      template,
    }
  }
  return {
    outcome: 'success',
    error: null,
    document,
    templateId,
    dealId,
    template,
    sourceSnapshot: (row.source_snapshot as Record<string, unknown> | null) ?? null,
  }
}

export async function evaluateAgreementCompletion(
  documentId: string,
  eventId: string,
  deps: AgreementCompletionDeps,
): Promise<AgreementCompletionResult> {
  return deps.run(async (tx) => {
    const ctx = await resolveAgreementDocument(documentId, deps.execute)
    if (ctx.outcome !== 'success') {
      return {
        outcome: ctx.outcome,
        error: ctx.error,
        verdict: INCOMPLETE,
        shouldEmit: false,
        document: ctx.document,
        templateId: ctx.templateId,
        dealId: ctx.dealId,
        eventId: null,
      }
    }
    const document = ctx.document!
    const templateId = ctx.templateId!
    const dealId = ctx.dealId
    const issuedVersion = document.issuedVersion
    // PARTICIPANT CARDINALITY: required slots come ONLY from the IMMUTABLE issued
    // snapshot (source_snapshot.issuedParticipants). Strict parse fails closed on a
    // missing/malformed/duplicate/ambiguous snapshot — a legacy document without a
    // valid snapshot returns validation_failure, never fabricated role-only slots.
    const parsed = parseIssuedParticipants(ctx.sourceSnapshot?.issuedParticipants)
    if (!parsed.ok) {
      return {
        outcome: 'validation_failure',
        error: `Document ${documentId} has an invalid issued-participant snapshot: ${parsed.error}`,
        verdict: INCOMPLETE,
        shouldEmit: false,
        document,
        templateId,
        dealId,
        eventId: null,
      }
    }
    const requiredSlots = resolveRequiredSlots(templateId, parsed.slots)
    if (requiredSlots.length === 0) {
      return {
        outcome: 'precondition_failure',
        error: `Template ${templateId} declares no required execution participants.`,
        verdict: INCOMPLETE,
        shouldEmit: false,
        document,
        templateId,
        dealId,
        eventId: null,
      }
    }

    const satisfiedSlotIds = [
      ...new Set([
        ...(await getCompletedExecutionSlots(documentId, deps.execute)),
        ...parseAppliedSignatureSlotIds(ctx.sourceSnapshot?.appliedSignatures),
      ]),
    ]

    const verdict = evaluateAgreementExecution({
      documentVersion: `${templateId}-v${issuedVersion}`,
      requiredSlots,
      satisfiedSlotIds,
      manuallyExecuted: false,
    })

    if (!verdict.fullyExecuted) {
      return {
        outcome: 'success',
        error: null,
        verdict,
        shouldEmit: false,
        document,
        templateId,
        dealId,
        eventId: null,
      }
    }

    const { recorded } = await claimAgreementExecution(tx, {
      documentId,
      issuedVersion,
      eventId,
      emittedAt: deps.now?.() ?? new Date(),
    })
    return {
      outcome: 'success',
      error: null,
      verdict,
      shouldEmit: recorded,
      document,
      templateId,
      dealId,
      eventId: recorded ? eventId : null,
    }
  })
}
