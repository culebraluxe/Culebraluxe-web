// ---------------------------------------------------------------------------
// TECH-DEBT-07 — release receipts that cannot be fabricated.
//
// The gap this closes: AgentRunEvidence.releaseEvidence is CONSUMED by the
// deploy/production_smoke adjudication in forge-role-mapping.ts, and nothing in
// the repo ever produced it. So every release-bearing story either blocked
// ("role did not deliver devops-receipt" after 2 attempts) or was at risk of
// someone typing a receipt to satisfy the gate.
//
// Two rules:
//   1. A receipt is DERIVED from a real deployment signal, never asserted. If
//      there is no signal, there is no receipt, and the deploy gate stays shut.
//   2. A receipt must be well-formed. A placeholder receiptId ("n/a", "test",
//      "tbd") or a non-SHA artifact is not evidence of anything.
//
// Pure module: no database, no network, no filesystem.
// ---------------------------------------------------------------------------

export type ReleaseReceiptKind = 'deployment' | 'production_verification' | 'integration'

/** Mirrors AgentRunEvidence.releaseEvidence exactly. */
export type ReleaseEvidence = {
  kind: ReleaseReceiptKind
  artifactSha: string
  receiptId: string
  success: boolean
}

/**
 * Values an operator or a model reaches for when it wants the gate to pass
 * without a deployment having happened. None of these are receipts.
 */
export const PLACEHOLDER_RECEIPT_IDS: ReadonlySet<string> = new Set([
  'n/a',
  'na',
  'none',
  'null',
  'nil',
  'tbd',
  'todo',
  'unknown',
  'placeholder',
  'synthetic',
  'fake',
  'mock',
  'dummy',
  'test',
  'manual',
  'waived',
  'x',
  '-',
])

const COMMIT_SHA = /^[0-9a-f]{7,40}$/i

export function isPlaceholderReceiptId(receiptId: string | null | undefined): boolean {
  const value = (receiptId ?? '').trim().toLowerCase()
  if (!value) return true
  if (PLACEHOLDER_RECEIPT_IDS.has(value)) return true
  // "none", "n/a (waived)", "tbd - see memo" all reduce to a placeholder word.
  const head = value.split(/[\s(,:—-]+/)[0]
  return PLACEHOLDER_RECEIPT_IDS.has(head)
}

export function isCommitSha(value: string | null | undefined): boolean {
  return COMMIT_SHA.test((value ?? '').trim())
}

export type ReceiptAssessment = { ok: boolean; reason: string | null }

/**
 * Validate a release receipt in isolation. FAIL CLOSED: an absent, malformed or
 * placeholder receipt is never acceptable evidence.
 *
 * 'integration' is the release-engineer attestation (TECH-DEBT-07, dialed back
 * 2026-09-12): the artifact sha is contained in the integration ref and a build
 * actually ran and exited 0. It is validated by the same rules as any other kind —
 * real sha, non-placeholder id, explicit success — and its `integration:` id prefix
 * keeps it distinguishable from a deployment receipt.
 */
export function assessReleaseReceipt(receipt: ReleaseEvidence | null | undefined): ReceiptAssessment {
  if (!receipt) return { ok: false, reason: 'no release receipt was provided' }
  if (
    receipt.kind !== 'deployment' &&
    receipt.kind !== 'production_verification' &&
    receipt.kind !== 'integration'
  ) {
    return { ok: false, reason: `release receipt kind is not recognized: ${JSON.stringify(receipt.kind)}` }
  }
  if (!isCommitSha(receipt.artifactSha)) {
    return {
      ok: false,
      reason: `release receipt artifactSha is not a commit sha: ${JSON.stringify(receipt.artifactSha ?? null)}`,
    }
  }
  if (isPlaceholderReceiptId(receipt.receiptId)) {
    return {
      ok: false,
      reason: `release receipt id is a placeholder, not a receipt: ${JSON.stringify(receipt.receiptId ?? null)}`,
    }
  }
  if (typeof receipt.success !== 'boolean') {
    return { ok: false, reason: 'release receipt success flag is not a boolean' }
  }
  return { ok: true, reason: null }
}

/**
 * The deploy gate's own question: does this receipt prove the PUBLISHED artifact
 * is the DEPLOYED artifact? Mirrors the adjudication in forge-role-mapping.ts so
 * a lane can fail early with a named reason instead of a silent HOLD.
 */
export function deploymentReceiptFailureReason(
  receipt: ReleaseEvidence | null | undefined,
  publishedSha: string | null | undefined,
): string | null {
  const assessment = assessReleaseReceipt(receipt)
  if (!assessment.ok) return assessment.reason
  if (!isCommitSha(publishedSha)) return 'no published artifact sha was recorded to compare against'
  if (receipt!.kind !== 'deployment') {
    return `deploy requires kind "deployment", got ${JSON.stringify(receipt!.kind)}`
  }
  if (!receipt!.success) return 'the deployment reported failure'
  if ((receipt!.artifactSha ?? '').trim().toLowerCase() !== (publishedSha ?? '').trim().toLowerCase()) {
    return (
      `the deployed artifact is not the published artifact: deployed ${receipt!.artifactSha}, ` +
      `published ${publishedSha}`
    )
  }
  return null
}

/** A real deployment signal, as produced by a provider (Vercel or other). */
export type DeploymentSignal = {
  provider: string
  deploymentId: string | null
  artifactSha: string | null
  state: string | null
}

/** Provider states that mean the deployment actually succeeded. */
export const DEPLOYMENT_SUCCESS_STATES: ReadonlySet<string> = new Set([
  'ready',
  'success',
  'succeeded',
  'deployed',
])

/**
 * Derive a receipt from a deployment signal. Returns null when there is no real
 * signal — no provider, no deployment id, no artifact sha, or a state that is
 * not a success. Null means the gate stays shut, which is the correct outcome.
 */
export function releaseReceiptFromDeploymentSignal(
  signal: DeploymentSignal | null | undefined,
): ReleaseEvidence | null {
  if (!signal) return null
  const provider = (signal.provider ?? '').trim()
  const deploymentId = (signal.deploymentId ?? '').trim()
  const artifactSha = (signal.artifactSha ?? '').trim()
  const state = (signal.state ?? '').trim().toLowerCase()
  if (!provider || !deploymentId || !isCommitSha(artifactSha)) return null
  if (!DEPLOYMENT_SUCCESS_STATES.has(state)) return null
  if (isPlaceholderReceiptId(deploymentId)) return null
  return {
    kind: 'deployment',
    artifactSha,
    receiptId: `${provider}:${deploymentId}`,
    success: true,
  }
}

/**
 * Is this story permitted to complete without a deployment? Only when the
 * deferral is RECORDED (batch-sliced rollout) — that is a deferral, not a claim.
 * An unrecorded absence is not a waiver.
 */
export function isRecordedDeploymentDeferral(deferredToBatch: number | null | undefined): boolean {
  return typeof deferredToBatch === 'number' && Number.isFinite(deferredToBatch) && deferredToBatch > 0
}

// ---------------------------------------------------------------------------
// ENG-FORGE-BATCH-RECEIPT-01 — the batch release receipt, derived from the
// actual release outcome and never asserted.
//
// A story may complete with its deployment DEFERRED to a release batch
// (`deploymentDeferredToBatch`). The batch release itself is run by hand
// (scripts/vercel-release-prod.sh prints the source commit it built and
// deployed). This is the other half of that deferral: the release records what
// it actually carried, so a sprint can be audited afterwards instead of
// remembered. No outcome means no receipt, and the record path refuses to write.
//
// Pure module addition: no database, no network, no filesystem, no clock.
// ---------------------------------------------------------------------------

/** The ACTUAL outcome of a batch release, as reported by the release itself. */
export type BatchReleaseOutcome = {
  /** The release batch (the sprint axis: `deployment_deferred_to_batch`). */
  batch: number | null
  /** The stories the release carried, already sliced by the caller. */
  storyIds: readonly string[]
  /** The commit the release built and deployed, or null when there was no release. */
  releasedSha: string | null
  /** When the release happened (ISO-8601), or null when unknown. */
  releasedAt: string | null
  /** True only when the release actually succeeded. */
  success: boolean
}

/** A durable batch release receipt: the batch, what it carried, the commit and when. */
export type BatchReleaseReceipt = {
  batch: number
  storyIds: string[]
  releasedSha: string
  releasedAt: string
}

function isoOrNull(value: string | null | undefined): string | null {
  if (value == null) return null
  const parsed = Date.parse(String(value))
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString()
}

/**
 * Derive a batch release receipt from the ACTUAL release outcome.
 *
 * Returns null — and therefore no receipt — when there was no real release: an
 * absent outcome, a failed release, no real commit sha, no positive batch, no
 * carried story, or no usable release time. A receipt is never asserted from the
 * deferral record; only a real outcome produces one.
 */
export function batchReleaseReceiptFromOutcome(
  outcome: BatchReleaseOutcome | null | undefined,
): BatchReleaseReceipt | null {
  if (!outcome) return null
  if (outcome.success !== true) return null
  if (!isRecordedDeploymentDeferral(outcome.batch)) return null
  if (!isCommitSha(outcome.releasedSha)) return null
  const releasedSha = (outcome.releasedSha ?? '').trim()
  if (isPlaceholderReceiptId(releasedSha)) return null
  const storyIds = [...new Set((outcome.storyIds ?? []).map((s) => String(s).trim()).filter(Boolean))]
  if (storyIds.length === 0) return null
  const releasedAt = isoOrNull(outcome.releasedAt)
  if (!releasedAt) return null
  return { batch: outcome.batch as number, storyIds, releasedSha, releasedAt }
}

/**
 * The record path's guard: the receipt, or a refusal. A caller that would write a
 * batch release receipt without a real release outcome gets an error rather than
 * a fabricated receipt.
 */
export function requireBatchReleaseReceipt(
  outcome: BatchReleaseOutcome | null | undefined,
): BatchReleaseReceipt {
  const receipt = batchReleaseReceiptFromOutcome(outcome)
  if (!receipt) {
    throw new Error(
      'refusing to record a batch release: the release outcome carries no real released commit',
    )
  }
  return receipt
}
