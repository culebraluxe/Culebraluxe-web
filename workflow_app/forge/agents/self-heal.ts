/**
 * ADD. Reason text rides ALONGSIDE `missing`, never inside it.
 * `missing` stays the retry hash (stable kind names).
 *
 * NOTE: the self-heal reason SIDECAR is Grok's design (buildSelfHealDirectiveWithReasons).
 * Its companion `retryHashKey` was NOT adopted: the live hash already exists and is
 * RICHER — `retryInputHash({ missReasons })` in forge-observer-seam.ts keys on the
 * miss plus the packet and assignment, which a miss-only key would silently lose.
 */
export function buildSelfHealDirectiveWithReasons(
  nodeId: string,
  missing: readonly string[],
  evidenceInstruction: string | null | undefined,
  rejectionReasons?: readonly string[],
): string {
  const fields = missing.length > 0 ? missing.join(', ') : '(unreported)'
  const why =
    rejectionReasons && rejectionReasons.length > 0
      ? `WHY (sidecar; not part of the retry hash): ${rejectionReasons.join('; ')}\n`
      : ''
  return (
    `SELF-HEAL REPROMPT (node ${nodeId}): this run was HELD because it did not deliver: ${fields}.\n` +
    why +
    (evidenceInstruction
      ? `The role's required structured output is: ${evidenceInstruction}\n`
      : '') +
    'Re-run this role. You MUST end your reply by emitting the required structured evidence so those fields are present and valid.'
  )
}
