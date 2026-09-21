/**
 * WHY A CLAIM WAS REFUSED, in words an operator can act on.
 *
 * The engine enforces ONE system-wide single-active work item, so a claim fails while any
 * other item is Claimed/Running/Paused. That refusal used to say only "could not claim",
 * which is unactionable: on 2026-09-13 it took a manual database query to learn that the
 * blocker was a leftover item, and `forge:clean` will not clear a claim younger than its
 * staleness cutoff by design. This turns the same refusal into a sentence naming the holder,
 * its age, and the command that resolves it.
 *
 * Pure: the caller supplies the rows and the clock.
 */
export type ClaimBlockerRow = {
  readonly id: string
  readonly storyId: string
  readonly role: string | null
  readonly state: string
  readonly claimedBy: string | null
  /** When the blocker last moved (updated_at), used for the age. */
  readonly updatedAt: string | null
}

export function describeClaimBlocker(input: {
  rows: readonly ClaimBlockerRow[]
  nowMs: number
  /** The staleness cutoff the recovery sweep uses, for the "stale" verdict. */
  staleMs: number
  /** The story the refused work belongs to, so the advice can be specific. */
  storyId: string
}): string {
  if (input.rows.length === 0) {
    // No holder visible: the lock was taken and released between the refusal and this read,
    // or another writer holds it outside these states. Say what is true rather than guess.
    return (
      'no work item is holding the single-active lock right now, so the refusal was ' +
      'transient — retry the run'
    )
  }

  const lines = input.rows.map((row) => {
    const ageMs = ageOf(row.updatedAt, input.nowMs)
    const stale = ageMs !== null && ageMs >= input.staleMs
    const age = ageMs === null ? 'age unknown' : `${Math.round(ageMs / 60_000)}m old`
    return (
      `${row.id} (${row.state}, role=${row.role ?? '?'}, story=${row.storyId}, ` +
      `held by ${row.claimedBy ?? 'nobody'}, ${age}${stale ? ', STALE' : ''})`
    )
  })

  const anyStale = input.rows.some((row) => {
    const ageMs = ageOf(row.updatedAt, input.nowMs)
    return ageMs !== null && ageMs >= input.staleMs
  })

  const advice = anyStale
    ? 'Run `pnpm forge:clean` to interrupt the stale claims, then retry.'
    : input.rows.some((row) => row.storyId === input.storyId)
      ? `This story already holds a claim: run \`pnpm forge:story:reset ${input.storyId} reset --force\` and retry.`
      : 'A live peer holds the lock: wait for it to finish, or confirm the owner is gone and then `pnpm forge:clean`.'

  return `the single-active lock is held by ${lines.join('; ')}. ${advice}`
}

/** Age in ms, or null when the timestamp is missing/unparseable (never a fabricated age). */
function ageOf(updatedAt: string | null, nowMs: number): number | null {
  if (!updatedAt) return null
  const parsed = Date.parse(updatedAt)
  if (!Number.isFinite(parsed)) return null
  return Math.max(0, nowMs - parsed)
}
