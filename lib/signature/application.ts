// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: application router/orchestrator.
//
// The router dispatches by configured provider, never by provider-specific
// command. Canonical state commits first; provider interaction occurs only
// after commit. Webhooks normalize at the provider seam before entering the
// canonical status command.
// ---------------------------------------------------------------------------

import { randomUUID } from 'node:crypto'
import type { CommandDispatcher, CommandResult } from '../commands/contracts'
import type { CommandEnvelope } from '../workflow/contracts'
import type { SignatureProvider } from './provider'
import type {
  SendSignatureRequestCommandInput,
  SignatureProviderEvent,
  SignatureRequest,
} from './contracts'
import {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
} from './contracts'
import {
  mapProviderStatus,
  neutralStatusForProviderEvent,
} from './status-mapping'
import type { SignatureReconciliationHandler } from './reconciliation'

export type SignatureApplicationDeps = {
  dispatcher: CommandDispatcher
  provider: SignatureProvider
  reconciler?: SignatureReconciliationHandler | null
  now?: () => Date
}

export type SignatureEnvelopeContext = {
  actorAppUserId?: string | null
  correlationId?: string | null
  causationId?: string | null
  commandId?: string
}

export type WebhookResult = {
  event: SignatureProviderEvent
  result: CommandResult
  reconciliation?: CommandResult | null
}

export class SignatureApplication {
  constructor(private readonly deps: SignatureApplicationDeps) {}

  private envelope(
    commandType: string,
    aggregateId: string | null,
    input: Record<string, unknown>,
    ctx: SignatureEnvelopeContext,
  ): CommandEnvelope {
    return {
      commandId: ctx.commandId ?? randomUUID(),
      commandType,
      actorAppUserId: ctx.actorAppUserId ?? null,
      aggregateType: 'signature_request',
      aggregateId,
      correlationId: ctx.correlationId ?? null,
      causationId: ctx.causationId ?? null,
      requestedAt: (this.deps.now?.() ?? new Date()).toISOString(),
      input,
    }
  }

  private withoutCommandId(ctx: SignatureEnvelopeContext): SignatureEnvelopeContext {
    const { commandId: _dropped, ...rest } = ctx
    return rest
  }

  /** Reconcile the neutral completion event emitted by a freshly-applied status. */
  private async reconcileIfCompleted(result: CommandResult): Promise<CommandResult | null> {
    if (!this.deps.reconciler) return null
    const event = result.emittedEvents.find(
      (e) => e.eventType === 'SIGNATURE_REQUEST_COMPLETED',
    )
    if (!event) return null
    try {
      return await this.deps.reconciler.onCompletedEvent(event)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return {
        commandId: event.eventId,
        outcome: 'conflict',
        emittedEvents: [],
        aggregateId: event.aggregateId,
        message: `Signed-artifact reconciliation failed (retry later): ${message}`,
        replayed: false,
        error: { code: 'reconciliation_failed', message, retryable: true },
      }
    }
  }

  /**
   * A provider can redeliver Completed after the canonical status has already
   * committed. In that case the status command emits no second completion
   * event, but artifact reconciliation may still need retrying (for example a
   * transient PDF/audit download failure). This helper retries reconciliation
   * only when the provider has just proven Completed and the status command
   * itself succeeded. The DB reconciler is independently receipt-backed and
   * guarded by signed_media_id, so repeated calls are safe.
   */
  private async reconcileCompletedObservation(
    result: CommandResult,
    signatureRequestId: string,
    providerProvesCompleted: boolean,
  ): Promise<CommandResult | null> {
    const emitted = await this.reconcileIfCompleted(result)
    if (emitted) return emitted
    if (!providerProvesCompleted || !this.deps.reconciler || result.outcome !== 'success') {
      return null
    }
    try {
      return await this.deps.reconciler.retryCompletedRequest(
        signatureRequestId,
        randomUUID(),
        (this.deps.now?.() ?? new Date()).toISOString(),
      )
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return {
        commandId: randomUUID(),
        outcome: 'conflict',
        emittedEvents: [],
        aggregateId: signatureRequestId,
        message: `Signed-artifact reconciliation retry failed: ${message}`,
        replayed: false,
        error: { code: 'reconciliation_failed', message, retryable: true },
      }
    }
  }

  /**
   * Send end-to-end: commit the neutral request, dispatch provider delivery,
   * then record the provider-observed neutral envelope status.
   */
  async send(
    input: SendSignatureRequestCommandInput,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<CommandResult> {
    const sendResult = await this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_SEND, null, {
        transactionDocumentId: input.transactionDocumentId,
        recipients: input.recipients,
        message: input.message ?? null,
        createdByUserId: input.createdByUserId ?? null,
        executionRole: input.executionRole ?? null,
        executionSlotId: input.executionSlotId ?? null,
        slotRecipientEmail: input.slotRecipientEmail ?? null,
        signatureRole: input.signatureRole ?? input.executionRole ?? null,
        completionRecipientEmails: input.completionRecipientEmails ?? [],
      }, ctx),
    )
    if (sendResult.outcome !== 'success') return sendResult
    const request = (sendResult.value as { signatureRequest: SignatureRequest }).signatureRequest

    const delivery = await this.deps.provider.send({
      signatureRequestId: request.id,
      transactionDocumentId: request.transactionDocumentId,
      recipients: input.recipients,
      message: request.message,
      signatureRole: input.signatureRole ?? input.executionRole ?? null,
      signatureSlotId: input.executionSlotId ?? null,
      completionRecipientEmails: input.completionRecipientEmails ?? [],
    })
    const target = mapProviderStatus(this.deps.provider.name, delivery.providerStatus)

    const statusResult = await this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_STATUS, request.id, {
        signatureRequestId: request.id,
        targetStatus: target,
      }, this.withoutCommandId(ctx)),
    )
    await this.reconcileCompletedObservation(statusResult, request.id, target === 'completed')
    return statusResult
  }

  /** Poll the provider and converge canonical envelope state. */
  async refreshStatus(
    signatureRequestId: string,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<CommandResult> {
    const observed = await this.deps.provider.status(signatureRequestId)
    const result = await this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_STATUS, signatureRequestId, {
        signatureRequestId,
        targetStatus: observed.status,
      }, ctx),
    )
    await this.reconcileCompletedObservation(
      result,
      signatureRequestId,
      observed.status === 'completed',
    )
    return result
  }

  /** Cancel canonical state, then revoke provider envelope after commit. */
  async cancel(
    signatureRequestId: string,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<CommandResult> {
    const result = await this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_CANCEL, signatureRequestId, {
        signatureRequestId,
      }, ctx),
    )
    if (result.outcome === 'success') {
      await this.deps.provider.cancel(signatureRequestId)
    }
    return result
  }

  async decline(
    signatureRequestId: string,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<CommandResult> {
    return this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_DECLINE, signatureRequestId, {
        signatureRequestId,
      }, ctx),
    )
  }

  /**
   * Verify/normalize the webhook, apply its neutral envelope status, then make
   * completion reconciliation part of webhook acknowledgement. A completed
   * provider event is acknowledged only after the final PDF/audit has either
   * reconciled successfully or is already reconciled. A transient artifact
   * failure throws so the route returns non-2xx and BoldSign can redeliver.
   */
  async handleWebhook(
    payload: unknown,
    signature: string,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<WebhookResult> {
    const verification = await this.deps.provider.verifyWebhook(payload, signature)
    const target = neutralStatusForProviderEvent(verification.event)
    const result = await this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_STATUS, verification.signatureRequestId, {
        signatureRequestId: verification.signatureRequestId,
        targetStatus: target,
      }, ctx),
    )
    const reconciliation = await this.reconcileCompletedObservation(
      result,
      verification.signatureRequestId,
      target === 'completed',
    )
    if (
      target === 'completed' &&
      reconciliation &&
      reconciliation.outcome !== 'success'
    ) {
      throw new Error(
        reconciliation.message ?? 'Completed signature artifact reconciliation failed.',
      )
    }
    return { event: verification.event, result, reconciliation }
  }
}
