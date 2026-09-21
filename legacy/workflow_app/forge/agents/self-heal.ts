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
  priorReply?: string | null,
): string {
  const fields = missing.length > 0 ? missing.join(', ') : '(unreported)'
  const why =
    rejectionReasons && rejectionReasons.length > 0
      ? `WHY (sidecar; not part of the retry hash): ${rejectionReasons.join('; ')}\n`
      : ''
  // THE MODEL'S OWN LAST ANSWER, fed back. A retry used to re-derive from scratch,
  // and on 2026-09-13 the re-derivation was WORSE than the answer it replaced: attempt
  // one produced a sound SOLO plan, the retry produced a HOLD, and the HOLD won. A
  // retry must REPAIR, not rewrite — so it gets its own prior output to correct.
  // The tail is kept, not the head: the contract lives at the END of a reply.
  const prior = (priorReply ?? '').trim()
  const priorBlock = prior
    ? `YOUR PREVIOUS REPLY (repair this; do NOT start over):\n---\n${
        prior.length > 4000 ? prior.slice(prior.length - 4000) : prior
      }\n---\n`
    : ''
  return (
    `SELF-HEAL REPROMPT (node ${nodeId}): this run was HELD because it did not deliver: ${fields}.\n` +
    why +
    priorBlock +
    (evidenceInstruction
      ? `The role's required structured output is: ${evidenceInstruction}\n`
      : '') +
    'Re-run this role. Fix ONLY what is missing above, and end your reply by emitting the required structured evidence so those fields are present and valid.'
  )
}
