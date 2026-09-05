export type ForgeStaleClaim = {
  taskId: string
  nodeId: string
  status: string
  claimedAtMs: number | null
}

export function isForgeClaimStale(
  claim: ForgeStaleClaim,
  nowMs: number,
  staleAfterMs: number,
): boolean {
  if (claim.status === 'ready') return false
  if (claim.claimedAtMs === null) return false
  if (!Number.isFinite(staleAfterMs) || staleAfterMs <= 0) return false
  return nowMs - claim.claimedAtMs >= staleAfterMs
}

export function staleAfterMsFromEnv(
  raw: string | null | undefined = process.env.AGENT_WORKER_STALE_AFTER_MINUTES,
): number {
  const minutes = Number(raw ?? 60)
  if (!Number.isFinite(minutes) || minutes <= 0) return 60 * 60_000
  return minutes * 60_000
}

export function selectStaleForgeClaims(
  claims: ForgeStaleClaim[],
  nowMs: number,
  staleAfterMs: number,
): ForgeStaleClaim[] {
  return claims.filter((claim) => isForgeClaimStale(claim, nowMs, staleAfterMs))
}
