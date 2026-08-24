import { getTemplate } from '../forms/template-registry'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import {
  claimAgreementExecution,
  getCompletedExecutionRoles,
} from '../../db/agreement-execution'
import {
  evaluateAgreementExecution,
  resolveRequiredExecutionRoles,
  type AgreementExecutionVerdict,
} from './execution'

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
// DURABILITY REPAIR: the whole evaluation runs through the caller-supplied
// TxRunner (`run`) — for the canonical command path this is the dispatcher's
// transaction (`ctx.run`), so the marker insert (`claimAgreementExecution`)
// commits atomically with the command receipt and the outbox row. It never
// opens a nested independent `neonTx`. A rolled-back command transaction
// leaves neither marker nor event.
// ---------------------------------------------------------------------------

export type AgreementCompletionResult = {
  verdict: AgreementExecutionVerdict
  /** True ONLY on the first time this document version is judged fully executed. */
  shouldEmit: boolean
  document: { documentId: string; issuedVersion: number } | null
  /** Template identity of the immutable issued document (for the event payload). */
  templateId: string | null
  /** Deal linkage of the immutable issued document (may be null pre-Deal). */
  dealId: string | null
}

export type AgreementCompletionDeps = {
  execute: QueryExecutor
  /** REQUIRED — the interactive transaction the marker participates in. */
  run: TxRunner
  now?: () => Date
}

export async function evaluateAgreementCompletion(
  documentId: string,
  eventId: string,
  deps: AgreementCompletionDeps,
): Promise<AgreementCompletionResult> {
  return deps.run(async (tx) => {
    const rows = await deps.execute`
      select template_id, issued_version, deal_id
      from transaction_document
      where id = ${documentId}
      limit 1
    `
    const row = rows[0] as
      | { template_id?: unknown; issued_version?: unknown; deal_id?: unknown }
      | undefined
    const templateId = typeof row?.template_id === 'string' ? row.template_id : ''
    const issuedVersion = Number(row?.issued_version ?? 0)
    const dealId = typeof row?.deal_id === 'string' ? row.deal_id : null
    if (issuedVersion < 1) {
      return {
        verdict: {
          fullyExecuted: false,
          missingRoles: [],
          reason: 'missing_required_roles',
        },
        shouldEmit: false,
        document: { documentId, issuedVersion: 0 },
        templateId: templateId || null,
        dealId,
      }
    }

    const template = getTemplate(templateId)
    const declaredRoles = (template?.signatureGroups ?? []).map((group) => group.role)
    const requiredRoles = resolveRequiredExecutionRoles(templateId, declaredRoles)
    const satisfiedRoles = await getCompletedExecutionRoles(documentId, deps.execute)

    const verdict = evaluateAgreementExecution({
      documentVersion: `${templateId}-v${issuedVersion}`,
      requiredRoles,
      satisfiedRoles,
      manuallyExecuted: false,
    })

    const document = { documentId, issuedVersion }
    if (!verdict.fullyExecuted) {
      return { verdict, shouldEmit: false, document, templateId: templateId || null, dealId }
    }

    const { recorded } = await claimAgreementExecution(tx, {
      documentId,
      issuedVersion,
      eventId,
      emittedAt: deps.now?.() ?? new Date(),
    })
    return { verdict, shouldEmit: recorded, document, templateId: templateId || null, dealId }
  })
}
