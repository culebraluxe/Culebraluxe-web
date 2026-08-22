// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: provider status -> neutral status.
//
// The STATUS-MAPPING BOUNDARY. Every provider observation (send outcome,
// status poll, webhook) is normalized onto the neutral model here, so
// provider semantics can never leak into canonical domain models. Provider
// adapters (DOC-04 BoldSign) extend `mapProviderStatus` with their own
// vocabulary; anything unknown fails closed to neutral 'error'.
// ---------------------------------------------------------------------------

import type {
  SignatureProviderEvent,
  SignatureRequestStatus,
} from './contracts'
import { SIGNATURE_PROVIDER_EVENTS } from './contracts'

export const FAKE_SIGNATURE_PROVIDER = 'fake-sign'

export const FAKE_PROVIDER_STATUSES = [
  'fake_requested',
  'fake_sent',
  'fake_viewed',
  'fake_signed',
  'fake_completed',
  'fake_declined',
  'fake_voided',
  'fake_expired',
  'fake_error',
] as const

export type FakeProviderStatus = (typeof FAKE_PROVIDER_STATUSES)[number]

const FAKE_STATUS_TO_NEUTRAL: Record<FakeProviderStatus, SignatureRequestStatus> = {
  fake_requested: 'requested',
  fake_sent: 'sent',
  fake_viewed: 'viewed',
  fake_signed: 'signed',
  fake_completed: 'completed',
  fake_declined: 'declined',
  fake_voided: 'voided',
  fake_expired: 'expired',
  fake_error: 'error',
}

export function isFakeProviderStatus(value: string): value is FakeProviderStatus {
  return (FAKE_PROVIDER_STATUSES as readonly string[]).includes(value)
}

/**
 * Map a raw provider status onto the neutral model. The provider's name picks
 * its vocabulary; an unknown status (or unknown provider) fails closed to
 * 'error' so it is never cast to a false success.
 *
 * DOC-04 adds a BoldSign branch here behind the same function.
 */
export function mapProviderStatus(
  providerName: string,
  providerStatus: string,
): SignatureRequestStatus {
  if (providerName === FAKE_SIGNATURE_PROVIDER && isFakeProviderStatus(providerStatus)) {
    return FAKE_STATUS_TO_NEUTRAL[providerStatus]
  }
  return 'error'
}

/**
 * Normalize a neutral webhook event onto the neutral status it reports. The
 * event vocabulary IS the neutral status vocabulary minus 'requested' (a
 * provider webhook never reports the pre-send state), so this is identity on
 * its input type — it exists to make the boundary explicit and total.
 */
export function neutralStatusForProviderEvent(
  event: SignatureProviderEvent,
): SignatureProviderEvent {
  return event
}

export function isSignatureProviderEvent(
  value: string,
): value is SignatureProviderEvent {
  return (SIGNATURE_PROVIDER_EVENTS as readonly string[]).includes(value)
}
