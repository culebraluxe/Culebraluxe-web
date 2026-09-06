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
// New canonical P&S documents are Contract-linked. Legacy Deal linkage is kept
// only so already-issued historical agreement evidence can still be evaluated;
// consumers of the new PR-PNS path must use Contract lineage.
// ---------------------------------------------------------------------------

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
  outcome: AgreementCompletionOutcome
  error: string | null
  verdict: AgreementExecutionVerdict
  shouldEmit: boolean
  document: { documentId: string; issuedVersion: number } | null
  templateId: string | null
  /** Canonical Contract linkage for new P&S execution. */
  contractId: string | null
  /** Legacy linkage retained only for historical compatibility. */
  dealId: string | null
  eventId: string | null
}

export type AgreementCompletionDeps = {
  execute: QueryExecutor
  run: TxRunner
  now?: () => Date
}

export type AgreementDocumentContext = {
  outcome: 'success' | AgreementCompletionOutcome
  error: string | null
  document: { documentId: string; issuedVersion: number } | null
  templateId: string | null
  contractId: string | null
  dealId: string | null
  template: ReturnType<typeof getTemplate>
  sourceSnapshot?: Record<string, unknown> | null
}

export async function resolveAgreementDocument(
  documentId: string,
  execute: QueryExecutor,
): Promise<AgreementDocumentContext> {
  const rows = await execute`
    select id, template_id, template_version, issued_version,
           contract_id, deal_id, document_type, source_snapshot
    from transaction_document
    where id = ${documentId}
    limit 1
  `
  const row = rows[0] as
    | {
        id?: unknown
        template_id?: unknown
        template_version?: unknown
        issued_version?: unknown
        contract_id?: unknown
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
      contractId: null,
      dealId: null,
      template: null,
    }
  }

  const templateId = typeof row.template_id === 'string' ? row.template_id : ''
  const templateVersion = Number(row.template_version ?? 0)
  const issuedVersion = Number(row.issued_version ?? 0)
  const contractId = typeof row.contract_id === 'string' ? row.contract_id : null
  const dealId = typeof row.deal_id === 'string' ? row.deal_id : null
  const documentType = typeof row.document_type === 'string' ? row.document_type : ''
  const document = { documentId, issuedVersion }

  if (!Number.isInteger(issuedVersion) || issuedVersion < 1) {
    return {
      outcome: 'validation_failure',
      error: `Invalid issued_version for ${documentId}: ${issuedVersion}.`,
      document: { documentId, issuedVersion: 0 },
      templateId: templateId || null,
      contractId,
      dealId,
      template: null,
    }
  }

  const template = getTemplate(templateId, templateVersion)
  if (!template) {
    return {
      outcome: 'validation_failure',
      error: `Unknown template ${templateId || '(none)'} v${templateVersion || 0} for document ${documentId}.`,
      document,
      templateId: templateId || null,
      contractId,
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
      contractId,
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
      contractId,
      dealId,
      template,
    }
  }

  // CONTRACT-CUT: new Contract-owned P&S carries contract_id and deal_id=null.
  // Keep accepting historical Deal-linked rows here so already-issued evidence
  // remains readable, but downstream CRM26 requires Contract lineage.
  if (!contractId && !dealId) {
    return {
      outcome: 'precondition_failure',
      error: `Document ${documentId} has neither Contract nor legacy Deal linkage required for execution.`,
      document,
      templateId,
      contractId,
      dealId,
      template,
    }
  }

  return {
    outcome: 'success',
    error: null,
    document,
    templateId,
    contractId,
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
        contractId: ctx.contractId,
        dealId: ctx.dealId,
        eventId: null,
      }
    }

    const document = ctx.document!
    const templateId = ctx.templateId!
    const contractId = ctx.contractId
    const dealId = ctx.dealId
    const issuedVersion = document.issuedVersion

    const parsed = parseIssuedParticipants(ctx.sourceSnapshot?.issuedParticipants)
    if (!parsed.ok) {
      return {
        outcome: 'validation_failure',
        error: `Document ${documentId} has an invalid issued-participant snapshot: ${parsed.error}`,
        verdict: INCOMPLETE,
        shouldEmit: false,
        document,
        templateId,
        contractId,
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
        contractId,
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
        contractId,
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
      contractId,
      dealId,
      eventId: recorded ? eventId : null,
    }
  })
}
