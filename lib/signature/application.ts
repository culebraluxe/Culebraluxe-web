// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: application router/orchestrator.
//
// "The router dispatches by configured provider, never by provider-specific
// command." UI / API / webhook callers invoke THIS class; it composes the
// neutral commands (canonical, receipt-backed) with provider dispatch through
// the configured SignatureProvider. Ordering invariant:
//
//   1. record/transition canonical state via a command (commits first,
//      idempotent via claim-first receipts);
//   2. THEN dispatch to the provider — strictly after commit, never inside a
//      domain service (db/*) and never inside a transaction.
//
// The webhook path normalizes at the seam FIRST (provider.verifyWebhook ->
// neutral {event, signatureRequestId}), then records the neutral status via
// the same canonical command — a webhook handler never writes straight to
// transaction_document (rejected design).
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

export type SignatureApplicationDeps = {
  /** The canonical command seam (all four neutral commands route here). */
  dispatcher: CommandDispatcher
  /** The CONFIGURED provider — the router dispatches by it, never by
   *  provider-specific command. */
  provider: SignatureProvider
  /** Application clock (injectable for deterministic tests). */
  now?: () => Date
}

/** Envelope provenance: actor + correlation/causation chain. */
export type SignatureEnvelopeContext = {
  actorAppUserId?: string | null
  correlationId?: string | null
  causationId?: string | null
  /** Explicit commandId for replay-safe callers; default: fresh UUID. */
  commandId?: string
}

export type WebhookResult = {
  /** The normalized neutral event (never a provider payload). */
  event: SignatureProviderEvent
  result: CommandResult
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

  /** Drop the explicit commandId so a FOLLOW-UP command in the same flow gets
   *  its own receipt (never a replay of the first command). */
  private withoutCommandId(ctx: SignatureEnvelopeContext): SignatureEnvelopeContext {
    const { commandId: _dropped, ...rest } = ctx
    return rest
  }

  /**
   * Send a signature request end-to-end: record the neutral request
   * (idempotent — an existing active request for the transaction document is
   * returned, never a duplicate), then dispatch to the provider strictly after
   * the send command committed, then record the provider's observed status
   * (mapped to neutral at the seam). Provider failures land as neutral 'error'
   * and are later reconciled by status polls / webhooks (DOC-05).
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
      }, ctx),
    )
    if (sendResult.outcome !== 'success') return sendResult
    const request = (sendResult.value as { signatureRequest: SignatureRequest }).signatureRequest

    // Provider dispatch AFTER the send command committed (never inside a
    // transaction, never inside a domain service).
    const delivery = await this.deps.provider.send({
      signatureRequestId: request.id,
      transactionDocumentId: request.transactionDocumentId,
      recipients: input.recipients,
      message: request.message,
    })
    const target = mapProviderStatus(this.deps.provider.name, delivery.providerStatus)

    return this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_STATUS, request.id, {
        signatureRequestId: request.id,
        targetStatus: target,
      }, this.withoutCommandId(ctx)),
    )
  }

  /** Poll the provider and apply the observed status (already mapped to
   *  neutral at the seam by the adapter). */
  async refreshStatus(
    signatureRequestId: string,
    ctx: SignatureEnvelopeContext = {},
  ): Promise<CommandResult> {
    const observed = await this.deps.provider.status(signatureRequestId)
    return this.deps.dispatcher.execute(
      this.envelope(SIGNATURE_REQUEST_STATUS, signatureRequestId, {
        signatureRequestId,
        targetStatus: observed.status,
      }, ctx),
    )
  }

  /** Cancel: record neutral 'voided' (idempotent), then best-effort cancel at
   *  the provider AFTER commit. Reconciliation converges on divergence. */
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

  /** Record a recipient decline -> neutral 'declined' (idempotent). The
   *  provider learns through its own system (webhook) — the provider interface
   *  has no decline call. */
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
   * Webhook entrypoint: verify + normalize AT THE SEAM FIRST
   * (provider.verifyWebhook -> neutral {event, signatureRequestId}), then
   * record the neutral status through the canonical command. Invalid
   * signatures reject (provider throws). The neutral events
   * (sent/completed/declined/voided) are emitted by the command with the
   * correlation/causation supplied here.
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
    return { event: verification.event, result }
  }
}
