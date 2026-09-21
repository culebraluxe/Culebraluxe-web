// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-08 — independent verification context + authoritative
// anchors.
//
// Part A: QA context is compiled independently from canonical sources (story,
// acceptance criteria, frozen candidate SHA, required evidence) and SHALL NOT
// be derived from Smith's conversational reasoning.
//
// Part B: an ANCHOR is evidence produced by an authoritative external system or
// deterministic mechanism. AGENT TESTIMONY CANNOT SATISFY AN ANCHOR REQUIREMENT
// — "tests pass" is not the test anchor; the actual exit code is.
//
// Pure + DB-free.
// ---------------------------------------------------------------------------

export type AnchorKind = 'git' | 'test' | 'db' | 'deployment' | 'http' | 'artifact' | 'policy'

export type EvidenceSource = 'system' | 'agent'

export type AnchorEvidence = {
  kind: AnchorKind
  source: EvidenceSource
  exitCode?: number | null
  verifiedSha?: string | null
  detail?: string | null
}

export type VerificationContext = {
  storyId: string
  /** Frozen candidate SHA from authoritative state (never a worker's claim). */
  candidateSha: string | null
  acceptanceCriteria: string[]
  testScope: string[]
  requiredAnchors: AnchorKind[]
  /** QA context is independent; Smith's conversational narrative is excluded. */
  smithNarrativeExcluded: true
}

export function compileVerificationContext(input: {
  storyId: string
  candidateSha: string | null
  acceptanceCriteria: string[]
  testScope: string[]
  requiredAnchors: AnchorKind[]
}): VerificationContext {
  return { ...input, smithNarrativeExcluded: true }
}

export type VerificationResult = {
  passed: boolean
  blockers: string[]
}

/**
 * A required anchor is satisfied only by SYSTEM evidence of that kind. Agent
 * testimony is never sufficient. A test anchor requires exit code 0; any anchor
 * carrying a verified SHA must match the frozen candidate SHA.
 */
export function evaluateVerification(input: {
  requiredAnchors: AnchorKind[]
  evidence: AnchorEvidence[]
  candidateSha: string | null
}): VerificationResult {
  const blockers: string[] = []
  for (const kind of input.requiredAnchors) {
    const system = input.evidence.find((e) => e.kind === kind && e.source === 'system')
    const agentClaims = input.evidence.some((e) => e.kind === kind && e.source === 'agent')
    if (!system) {
      blockers.push(
        agentClaims
          ? `${kind} anchor: agent testimony cannot satisfy an anchor requirement`
          : `missing ${kind} anchor (no evidence)`,
      )
      continue
    }
    if (kind === 'test' && (system.exitCode ?? 1) !== 0) {
      blockers.push(`test anchor failed (exit ${system.exitCode})`)
    }
    if (system.verifiedSha && input.candidateSha && system.verifiedSha !== input.candidateSha) {
      blockers.push(`${kind} anchor verified wrong SHA (${system.verifiedSha}, expected ${input.candidateSha})`)
    }
  }
  return { passed: blockers.length === 0, blockers }
}
