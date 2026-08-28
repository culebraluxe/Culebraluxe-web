// ---------------------------------------------------------------------------
// DOC-05 — Signed Document Reconciliation: the NEUTRAL completed-event
// subscriber.
//
// "A reconciliation handler that subscribes to NEUTRAL
// signature.request.completed events — never provider webhooks directly."
// This class IS that subscriber: it accepts a neutral DomainEvent (the
// SIGNATURE_REQUEST_COMPLETED fact emitted by the canonical status command at
// the DOC-03 seam) and drives the canonical reconciliation service
// (db/signature-reconciliation.ts). It is invoked strictly AFTER the command
// that emitted the event has committed — the router
// (lib/signature/application.ts) calls it post-commit; a future durable outbox
// loop can call the same onCompletedEvent with persisted neutral events.
// ---------------------------------------------------------------------------

import type { CommandResult, DomainEvent } from '../workflow/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import {
  reconcileCompletedSignatureRequest,
  type ReconcileCompletedSignatureRequestDeps,
} from '../../db/signature-reconciliation'
import type { SignatureProvider } from './provider'
import type { AgreementCompletionResult } from '../agreements/completion'

export type SignatureReconciliationHandlerDeps = {
  provider: SignatureProvider
  run: TxRunner
  execute?: QueryExecutor
  now?: () => Date
  evaluateAgreement?: (
    documentId: string,
    eventId: string,
  ) => Promise<AgreementCompletionResult>
}

export class SignatureReconciliationHandler {
  constructor(private readonly deps: SignatureReconciliationHandlerDeps) {}

  private async reconcileRequest(
    signatureRequestId: string,
    eventId: string,
    occurredAt?: string | null,
    transactionDocumentId?: string | null,
  ): Promise<CommandResult> {
    const deps: ReconcileCompletedSignatureRequestDeps = {
      run: this.deps.run,
      execute: this.deps.execute,
      downloadSignedArtifact: (requestId) => this.deps.provider.downloadSignedArtifact(requestId),
      downloadAuditTrail: (requestId) => this.deps.provider.downloadAuditTrail(requestId),
      now: this.deps.now,
    }
    const reconcile = await reconcileCompletedSignatureRequest(
      { eventId, signatureRequestId, occurredAt },
      deps,
    )

    // A replay may not carry the original event payload, but reconciliation's
    // provider-neutral result resolves the canonical transaction document from
    // signature_request. That makes completion recovery independent of the
    // original webhook body.
    const value = (reconcile.value ?? {}) as Record<string, unknown>
    const resolvedDocumentId =
      typeof transactionDocumentId === 'string' && transactionDocumentId !== ''
        ? transactionDocumentId
        : typeof value.documentId === 'string'
          ? value.documentId
          : null

    if (this.deps.evaluateAgreement && resolvedDocumentId) {
      try {
        const completion = await this.deps.evaluateAgreement(resolvedDocumentId, eventId)
        return {
          ...reconcile,
          value: { ...value, agreementCompletion: completion },
        }
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error)
        return {
          ...reconcile,
          message: [
            reconcile.message,
            `Agreement execution evaluation failed: ${detail}`,
          ]
            .filter(Boolean)
            .join(' '),
          value: { ...value, agreementCompletion: { shouldEmit: false } },
        }
      }
    }
    return reconcile
  }

  /**
   * Subscribe to a neutral event. Only SIGNATURE_REQUEST_COMPLETED events are
   * reconciled; any other neutral event is explicitly ignored.
   */
  async onCompletedEvent(event: DomainEvent): Promise<CommandResult> {
    if (event.eventType !== 'SIGNATURE_REQUEST_COMPLETED') {
      return {
        commandId: event.eventId,
        outcome: 'precondition_failure',
        emittedEvents: [],
        aggregateId: event.aggregateId,
        message:
          `Reconciliation subscribes only to SIGNATURE_REQUEST_COMPLETED; ` +
          `got '${event.eventType}'.`,
        replayed: false,
      }
    }
    const payload = event.payload as {
      signatureRequestId?: unknown
      transactionDocumentId?: unknown
    }
    if (typeof payload?.signatureRequestId !== 'string' || payload.signatureRequestId === '') {
      return {
        commandId: event.eventId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: event.aggregateId,
        message: 'SIGNATURE_REQUEST_COMPLETED event payload is missing signatureRequestId.',
        replayed: false,
      }
    }
    return this.reconcileRequest(
      payload.signatureRequestId,
      event.eventId,
      event.occurredAt,
      typeof payload.transactionDocumentId === 'string'
        ? payload.transactionDocumentId
        : null,
    )
  }

  /**
   * Retry a provider-proven completed request even when the canonical status
   * command is now a replay/no-op and emits no second DomainEvent. The caller
   * supplies a stable attempt id (normally the provider webhook event id or
   * poll command id). The underlying reconciliation has its own receipt and an
   * already-signed guard, so this is safe to invoke repeatedly.
   */
  async retryCompletedRequest(
    signatureRequestId: string,
    eventId: string,
    occurredAt?: string | null,
  ): Promise<CommandResult> {
    return this.reconcileRequest(signatureRequestId, eventId, occurredAt)
  }
}
