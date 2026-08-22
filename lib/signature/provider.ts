// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: the neutral SignatureProvider interface.
//
// Provider adapters (DOC-04 BoldSign) implement this interface; the application
// router (lib/signature/application.ts) dispatches by the CONFIGURED provider,
// never by provider-specific command. Everything here is neutral:
//   - send/status/cancel take and return neutral shapes; provider ids live in
//     the adapter's own provider table (DOC-04) and never cross the seam;
//   - status() returns the provider status ALREADY mapped to neutral;
//   - verifyWebhook(payload, signature) returns a neutral {event,
//     signatureRequestId} — webhook payloads are normalized at the seam.
// ---------------------------------------------------------------------------

import type {
  ProviderActionResult,
  ProviderSendRequest,
  ProviderSendResult,
  ProviderStatusResult,
  SignedArtifactDownload,
  WebhookVerificationResult,
} from './contracts'

export interface SignatureProvider {
  /** Stable provider identifier (picks the seam's status-mapping vocabulary). */
  readonly name: string

  /** Send a signature request to the provider. Neutral request in, raw
   *  provider status out (mapped to neutral at the seam). */
  send(request: ProviderSendRequest): Promise<ProviderSendResult>

  /** Current provider status for a neutral signature request id. */
  status(requestId: string): Promise<ProviderStatusResult>

  /** Cancel/void the request at the provider (best-effort; reconciliation
   *  converges via status polls / webhooks). */
  cancel(requestId: string): Promise<ProviderActionResult>

  /** Verify a webhook signature and normalize the payload to a neutral event +
   *  the NEUTRAL signature request id. Throws on invalid signatures. */
  verifyWebhook(
    payload: unknown,
    signature: string,
  ): Promise<WebhookVerificationResult>

  /**
   * DOC-05 — Download the FINAL signed artifact bytes for a neutral signature
   * request id, exactly once per reconciliation. The adapter resolves its own
   * provider envelope through its provider table (never a provider id across
   * the seam) and returns neutral bytes + storage metadata. Throws on
   * provider failures — the reconciliation retries later and keeps the
   * document in its current (sent) state.
   */
  downloadSignedArtifact(requestId: string): Promise<SignedArtifactDownload>
}
