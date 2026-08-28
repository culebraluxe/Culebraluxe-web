import type { IssuedExecutionSlot } from '../agreements/execution'
import type { SignatureRecipient } from '../signature/contracts'

export type SignatureEnvelopeResolution =
  | { ok: true; recipients: SignatureRecipient[] }
  | { ok: false; error: string }

/**
 * Resolve the one BoldSign envelope from the immutable issued document.
 * Locally-applied signatures (Lisa) are already execution evidence and are
 * deliberately excluded from the provider recipient set. No signer limit is
 * imposed: the issued snapshot owns cardinality.
 */
export function resolveSignatureEnvelopeRecipients(
  requiredSlots: readonly IssuedExecutionSlot[],
  appliedSlotIds: readonly string[],
): SignatureEnvelopeResolution {
  const applied = new Set(appliedSlotIds)
  const external = requiredSlots
    .filter((slot) => !applied.has(slot.slotId))
    .sort((a, b) => a.order - b.order || a.slotId.localeCompare(b.slotId))

  if (external.length === 0) {
    return { ok: false, error: 'The issued agreement has no external signer remaining.' }
  }

  const missing = external.find(
    (slot) => !slot.name.trim() || !slot.email?.trim(),
  )
  if (missing) {
    return {
      ok: false,
      error: `Execution slot '${missing.slotId}' requires a signer name and email address.`,
    }
  }

  return {
    ok: true,
    recipients: external.map((slot, index) => ({
      role: 'signer',
      name: slot.name.trim(),
      email: slot.email!.trim(),
      order: index + 1,
      executionRole: slot.role,
      executionSlotId: slot.slotId,
    })),
  }
}
