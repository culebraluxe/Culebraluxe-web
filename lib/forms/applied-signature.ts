import type { PdfPointRectangle } from './signature-anchors'

export const BROKER_SIGNATURE_TIME_ZONE = 'America/Puerto_Rico' as const
export const BROKER_SIGNATURE_CONSENT_BASIS =
  'authenticated-owner-issuance' as const
export const BROKER_SIGNATURE_DATE_SEMANTIC =
  'issuance-requested-at' as const

export type AppliedSignatureImageMimeType = 'image/png' | 'image/jpeg'

/**
 * Authorized signature material supplied to the canonical PDF compositor.
 * Image bytes are deliberately transient and must never be copied into the
 * issued source snapshot. The immutable PDF contains their rendered result;
 * the snapshot retains only provenance, checksum and resolved placement.
 */
export type FormAppliedSignature = {
  role: string
  slotId: string | null
  signerName: string
  credentialLine: string
  signerAppUserId: string
  imageBytes: Uint8Array
  imageMimeType: AppliedSignatureImageMimeType
  assetMediaId: string
  assetChecksumSha256: string
  appliedAt: string
  consentBasis: typeof BROKER_SIGNATURE_CONSENT_BASIS
  dateSemantic: typeof BROKER_SIGNATURE_DATE_SEMANTIC
}

export type AppliedSignatureEvidence = {
  role: string
  slotId: string | null
  signerName: string
  credentialLine: string
  signerAppUserId: string
  assetMediaId: string
  assetChecksumSha256: string
  appliedAt: string
  consentBasis: typeof BROKER_SIGNATURE_CONSENT_BASIS
  dateSemantic: typeof BROKER_SIGNATURE_DATE_SEMANTIC
  renderedDate: string
  pageIndex: number
  signatureRect: PdfPointRectangle
  dateRect: PdfPointRectangle
}

/** Format the issuance boundary in the brokerage's operating time zone. */
export function formatBrokerSignatureDate(instant: string): string {
  const date = new Date(instant)
  if (Number.isNaN(date.getTime())) {
    throw new Error('Broker signature appliedAt must be a valid ISO instant.')
  }
  return new Intl.DateTimeFormat('en-US', {
    timeZone: BROKER_SIGNATURE_TIME_ZONE,
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(date)
}

/**
 * Strictly recover locally-composed execution-slot evidence from an immutable
 * issued snapshot. Malformed entries are ignored and can never satisfy a slot.
 */
export function parseAppliedSignatureSlotIds(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  const slots = new Set<string>()
  for (const entry of value) {
    if (!entry || typeof entry !== 'object') continue
    const raw = entry as Record<string, unknown>
    if (
      (raw.role !== 'BUYER_BROKER' && raw.role !== 'SELLER_BROKER') ||
      typeof raw.slotId !== 'string' ||
      !raw.slotId.startsWith(`${raw.role}:`) ||
      typeof raw.signerName !== 'string' || !raw.signerName.trim() ||
      typeof raw.credentialLine !== 'string' || !raw.credentialLine.trim() ||
      typeof raw.signerAppUserId !== 'string' || !raw.signerAppUserId.trim() ||
      typeof raw.assetMediaId !== 'string' || !raw.assetMediaId.trim() ||
      !/^[0-9a-f]{64}$/i.test(String(raw.assetChecksumSha256 ?? '')) ||
      typeof raw.appliedAt !== 'string' ||
      Number.isNaN(new Date(raw.appliedAt).getTime()) ||
      raw.consentBasis !== BROKER_SIGNATURE_CONSENT_BASIS ||
      raw.dateSemantic !== BROKER_SIGNATURE_DATE_SEMANTIC ||
      typeof raw.renderedDate !== 'string' || !raw.renderedDate.trim() ||
      typeof raw.pageIndex !== 'number' ||
      !Number.isInteger(raw.pageIndex) || raw.pageIndex < 0 ||
      !validEvidenceRect(raw.signatureRect) ||
      !validEvidenceRect(raw.dateRect)
    ) {
      continue
    }
    slots.add(raw.slotId)
  }
  return [...slots]
}

function validEvidenceRect(value: unknown): value is PdfPointRectangle {
  if (!value || typeof value !== 'object') return false
  const raw = value as Record<string, unknown>
  return (
    typeof raw.x === 'number' && Number.isFinite(raw.x) && raw.x >= 0 &&
    typeof raw.y === 'number' && Number.isFinite(raw.y) && raw.y >= 0 &&
    typeof raw.width === 'number' && Number.isFinite(raw.width) && raw.width > 0 &&
    typeof raw.height === 'number' && Number.isFinite(raw.height) && raw.height > 0
  )
}
