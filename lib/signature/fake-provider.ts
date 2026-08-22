// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: fake/test provider.
//
// Proves the seam end-to-end WITHOUT any provider (DOC-04 plugs the real
// BoldSign adapter in behind the same SignatureProvider interface). The fake
// keeps provider-specific state (its own provider status per request) in its
// own in-memory store — the analog of the DOC-04 provider table — so no
// provider field ever crosses the seam or reaches a canonical model. The
// neutral signatureRequestId is its store key; provider ids never exist here.
//
// Webhook verification is modeled as a deterministic signature over the
// payload using a shared secret, so tests can exercise both valid and invalid
// webhook deliveries.
// ---------------------------------------------------------------------------

import type { SignatureProvider } from './provider'
import type {
  ProviderActionResult,
  ProviderSendRequest,
  ProviderSendResult,
  ProviderStatusResult,
  WebhookVerificationResult,
} from './contracts'
import {
  FAKE_SIGNATURE_PROVIDER,
  FAKE_PROVIDER_STATUSES,
  isFakeProviderStatus,
  isSignatureProviderEvent,
  mapProviderStatus,
  neutralStatusForProviderEvent,
  type FakeProviderStatus,
} from './status-mapping'

type FakeProviderRecord = {
  transactionDocumentId: string
  recipients: ProviderSendRequest['recipients']
  message: string | null
  providerStatus: FakeProviderStatus
}

export class FakeSignatureProvider implements SignatureProvider {
  readonly name = FAKE_SIGNATURE_PROVIDER

  private readonly requests = new Map<string, FakeProviderRecord>()
  private readonly sharedSecret: string
  private nextSendError: string | null = null

  constructor(options: { sharedSecret?: string } = {}) {
    this.sharedSecret = options.sharedSecret ?? 'fake-signing-secret'
  }

  // -- test-driving helpers (not part of the provider contract) --------------

  /** Simulate the provider system moving a request to a provider status. */
  setProviderStatus(requestId: string, providerStatus: FakeProviderStatus): void {
    const record = this.requests.get(requestId)
    if (!record) throw new Error(`Fake provider: unknown request ${requestId}.`)
    if (!isFakeProviderStatus(providerStatus)) {
      throw new Error(`Fake provider: unknown status ${String(providerStatus)}.`)
    }
    record.providerStatus = providerStatus
  }

  /** Make the next send fail (delivery error), then clear the flag. */
  failNextSendWith(error: string): void {
    this.nextSendError = error
  }

  /** Deterministic webhook signature over a payload (models provider signing). */
  signWebhookPayload(payload: {
    signatureRequestId: string
    providerStatus: string
  }): string {
    return `${this.sharedSecret}:${payload.signatureRequestId}:${payload.providerStatus}`
  }

  // -- SignatureProvider -----------------------------------------------------

  async send(request: ProviderSendRequest): Promise<ProviderSendResult> {
    const failure = this.nextSendError
    this.nextSendError = null
    if (failure) {
      return { ok: false, providerStatus: 'fake_error', error: failure }
    }
    this.requests.set(request.signatureRequestId, {
      transactionDocumentId: request.transactionDocumentId,
      recipients: request.recipients,
      message: request.message,
      providerStatus: 'fake_sent',
    })
    return { ok: true, providerStatus: 'fake_sent' }
  }

  async status(requestId: string): Promise<ProviderStatusResult> {
    const record = this.requests.get(requestId)
    if (!record) {
      return { status: 'error' }
    }
    // Mapped to neutral AT THE SEAM (status-mapping) — the caller never sees
    // a provider status string.
    return { status: mapProviderStatus(this.name, record.providerStatus) }
  }

  async cancel(requestId: string): Promise<ProviderActionResult> {
    const record = this.requests.get(requestId)
    if (!record) return { ok: false, error: `Unknown provider request: ${requestId}.` }
    if (record.providerStatus === 'fake_completed' || record.providerStatus === 'fake_declined') {
      return { ok: false, error: `Request ${requestId} is already terminal at the provider.` }
    }
    record.providerStatus = 'fake_voided'
    return { ok: true }
  }

  async verifyWebhook(
    payload: unknown,
    signature: string,
  ): Promise<WebhookVerificationResult> {
    const body = payload as { signatureRequestId?: string; providerStatus?: string }
    if (!body || typeof body !== 'object') {
      throw new Error('Fake provider: malformed webhook payload.')
    }
    const { signatureRequestId, providerStatus } = body
    if (typeof signatureRequestId !== 'string' || typeof providerStatus !== 'string') {
      throw new Error('Fake provider: webhook payload missing signatureRequestId/providerStatus.')
    }
    const expected = this.signWebhookPayload({ signatureRequestId, providerStatus })
    if (signature !== expected) {
      throw new Error('Fake provider: invalid webhook signature.')
    }
    const record = this.requests.get(signatureRequestId)
    if (!record) {
      throw new Error(`Fake provider: webhook for unknown request ${signatureRequestId}.`)
    }
    if (!isFakeProviderStatus(providerStatus)) {
      throw new Error(`Fake provider: webhook with unknown status ${providerStatus}.`)
    }
    record.providerStatus = providerStatus
    // Normalized at the seam: the neutral event for the provider status. The
    // pre-send state ('requested') is not a webhook event — the provider only
    // webhooks states it has moved into AFTER a send.
    const neutral = mapProviderStatus(this.name, providerStatus)
    if (!isSignatureProviderEvent(neutral)) {
      throw new Error(
        `Fake provider: no neutral webhook event for status ${providerStatus} (pre-send state is never webhooked).`,
      )
    }
    return { event: neutralStatusForProviderEvent(neutral), signatureRequestId }
  }
}

export { FAKE_PROVIDER_STATUSES, type FakeProviderStatus }
