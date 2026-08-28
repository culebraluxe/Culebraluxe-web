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
// loop (CRM-14I defer decision) can call the same onCompletedEvent with
// persisted neutral events.
//
// Provider interaction is confined to the ONE-TIME signed-artifact download
// through the configured SignatureProvider (DOC-04 adapter): the provider
// table lookup + download happen inside downloadSignedArtifact, never here,
// and never inside a database transaction. No provider state is ever written
// by reconciliation.
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
  /** The CONFIGURED provider — used ONLY for the one-time signed-artifact
   *  download (DOC-04). */
  provider: SignatureProvider
  /** The reconciliation transaction runner. */
  run: TxRunner
  /** Executor for pre-claim reads (default: lazy Neon client). */
  execute?: QueryExecutor
  /** Application clock (injectable for deterministic tests). */
  now?: () => Date
  /** CRM-27 — optional agreement-completion evaluator invoked after a neutral
   *  completed event, so a fully-executed agreement can be recognised exactly
   *  once. Injected (never constructed here) to keep this seam provider-free. */
  evaluateAgreement?: (
    documentId: string,
    eventId: string,
  ) => Promise<AgreementCompletionResult>
}

export class SignatureReconciliationHandler {
  constructor(private readonly deps: SignatureReconciliationHandlerDeps) {}

  /**
   * Subscribe to a neutral event. Only SIGNATURE_REQUEST_COMPLETED events are
   * reconciled; any other neutral event is explicitly ignored (a non-result,
   * never an error path). The event id is the command-receipt key, so a
   * replayed event is a no-op.
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
    const deps: ReconcileCompletedSignatureRequestDeps = {
      run: this.deps.run,
      execute: this.deps.execute,
      downloadSignedArtifact: (requestId) => this.deps.provider.downloadSignedArtifact(requestId),
      downloadAuditTrail: (requestId) => this.deps.provider.downloadAuditTrail(requestId),
      now: this.deps.now,
    }
    const reconcile = await reconcileCompletedSignatureRequest(
      {
        eventId: event.eventId,
        signatureRequestId: payload.signatureRequestId,
        occurredAt: event.occurredAt,
      },
      deps,
    )
    // CRM-27 — after the signed artifact is reconciled, evaluate whether this
    // completion makes the linked issued agreement version FULLY EXECUTED.
    // `shouldEmit` is exactly-once; the fact's actual dispatch is the caller's
    // (a later workflow/wiring seam). A failure here is non-fatal — the signed
    // artifact is already reconciled — and is surfaced on the result.
    const transactionDocumentId =
      typeof payload.transactionDocumentId === 'string'
        ? payload.transactionDocumentId
        : null
    if (this.deps.evaluateAgreement && transactionDocumentId) {
      try {
        const completion = await this.deps.evaluateAgreement(
          transactionDocumentId,
          event.eventId,
        )
        const value = (reconcile.value ?? {}) as Record<string, unknown>
        return {
          ...reconcile,
          value: { ...value, agreementCompletion: completion },
        }
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error)
        const value = (reconcile.value ?? {}) as Record<string, unknown>
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
}
