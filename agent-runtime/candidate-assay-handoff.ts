// ---------------------------------------------------------------------------
// ENG-FORGE-V4-10C — Exact Candidate Assay Handoff: shared invariants.
//
// V4-11 exposed a false-positive path: Smith created candidate
// `5498665...`, but the following Assay workspace was provisioned from
// `main@7b14c6b...`. The Assay's first packet command failed because the file
// did not exist in that checkout, yet the verifier run/story normalized to
// Complete 100%.
//
// The invariant is strict:
//
//   Smith candidate C -> Assay workspace base C -> Assay evidence about C
//   -> only then may C publish.
//
// This module owns ONLY the neutral predicates and evidence helpers that every
// boundary (enqueue/handoff, workspace provisioning, Assay terminal
// normalization, accepted-candidate publish) shares, so the semantics cannot
// drift between seams. No provider/model/Slack/swarm/lane-order and no schema
// change live here — this is pure decision logic over existing run/work-item
// evidence.
// ---------------------------------------------------------------------------

import {
  isArithmeticAssayPass,
  parseAssayArithmeticFacts,
} from './assay-arithmetic'

/** Every role that represents a terminal Assay/verification completion.
 *  The codebase has used `reviewer` (V3 Assay repair/terminal semantics) and
 *  `verifier` (the DEFAULT_LANES.assay binding role) for this lane; both are
 *  acceptance-gate roles and must never normalize a failed verification to
 *  Complete. */
export const ASSAY_TERMINAL_ROLES = ['reviewer', 'verifier'] as const

export function isAssayTerminalRole(
  role: string | null | undefined,
): boolean {
  const value = (role ?? '').trim().toLowerCase()
  return (ASSAY_TERMINAL_ROLES as readonly string[]).includes(value)
}

/**
 * Legacy failure/abort vocabulary for old Assay rows that contain no usable
 * test counters. New evidence with real counters never uses this scanner;
 * its verdict comes only from assay-arithmetic.ts.
 */
export const ASSAY_FAILURE_EVIDENCE =
  /\b(fail(?:ed|ure|ures|ing)?|violation|policy|error|missing|not found|no such file|no test files|does not exist|doesn't exist|command not found|cannot find|unresolvable|could not resolve|exit code [1-9]|nonzero)\b/i

/**
 * Legacy summaries sometimes contain zero counters but no complete arithmetic
 * evidence. Strip explicit zero-failure counters only for that legacy fallback.
 */
const ASSAY_ZERO_FAILURE_COUNTER =
  /\b(?:0\s+(?:fail(?:ed|ure|ures|ing)?|errors?)|(?:fail(?:ed|ure|ures|ing)?|errors?)\s*[:=]?\s*0)\b/gi

function assayFailureScanText(testsSummary: string | null | undefined): string {
  return (testsSummary ?? '')
    .replace(ASSAY_ZERO_FAILURE_COUNTER, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

/**
 * Clean Assay evidence is math, not prose:
 *   Complete
 *   AND every reported exit code == 0
 *   AND reported failed/error count == 0
 *   AND, when both exist, passed == total
 *   AND policy violation count == 0.
 *
 * Human wording cannot override real test counters. A non-zero exit always
 * fails mathematically. Exit-only evidence is not enough to suppress a real
 * legacy blocker such as "missing file", so that narrow compatibility case
 * still goes through the old fail-closed vocabulary scanner.
 */
export function isCleanAssayEvidence(input: {
  resultStatus?: string | null
  testsSummary?: string | null
}): boolean {
  if (!/^complete$/i.test((input.resultStatus ?? '').trim())) return false

  const facts = parseAssayArithmeticFacts(input.testsSummary)
  if (facts.exitCodes.some((code) => code !== 0)) return false
  if (facts.hasTestCounters) return isArithmeticAssayPass(facts)

  return !ASSAY_FAILURE_EVIDENCE.test(assayFailureScanText(input.testsSummary))
}

/** Normalize a 40-hex commit hash (git case-insensitive), or null. */
export function commitSha(value: string | null | undefined): string | null {
  const sha = (value ?? '').trim()
  return /^[0-9a-f]{40}$/i.test(sha) ? sha.toLowerCase() : null
}

/**
 * The Smith candidate for a story: the NEWEST run that recorded a commit.
 *
 * Runs are supplied newest-first (the `storyboard_story_run` read order). An
 * Assay/verification run itself never keeps a commit, so at Assay finish time
 * this transparently resolves to the Smith candidate run beneath it. A run
 * with a non-commit/non-hash value is never treated as a candidate.
 */
export function smithCandidateSha(
  runs: ReadonlyArray<{ commitHash?: string | null }>,
): string | null {
  for (const run of runs) {
    const sha = commitSha(run.commitHash)
    if (sha) return sha
  }
  return null
}

/**
 * Strict handoff reading: the candidate for a JUST-FINISHED lane is the
 * finished run's OWN commit (the newest run in the story), never an older
 * cycle's candidate. A builder run that finished without a commit has NO
 * candidate for this cycle even when an older candidate commit exists.
 */
export function finishedRunCandidateSha(
  runs: ReadonlyArray<{ commitHash?: string | null }>,
): string | null {
  const newest = runs[0]
  return newest ? commitSha(newest.commitHash) : null
}

/** One machine-scannable workspace evidence line (ENG-21 evidence format). */
export function workspaceEvidenceLine(w: {
  branchName: string
  worktreePath: string
  baseRef: string
  baseCommit: string
}): string {
  return `Execution workspace: branch=${w.branchName} worktree=${w.worktreePath} base=${w.baseRef}@${w.baseCommit}`
}

/** Append the workspace evidence line when a workspace exists and no line is
 *  already present (dedupes the adapter-written evidence). */
export function withWorkspaceEvidence(
  workspace: { branchName: string; worktreePath: string; baseRef: string; baseCommit: string } | null | undefined,
  notes: string,
): string {
  if (!workspace) return notes
  if (/Execution workspace:/m.test(notes)) return notes
  const trimmed = notes.trim()
  return trimmed
    ? `${trimmed}\n\n${workspaceEvidenceLine(workspace)}`
    : workspaceEvidenceLine(workspace)
}

/**
 * The base commit an Assay run actually executed against, read from its
 * workspace evidence line (`base=<ref>@<40-hex>`). Null when the run recorded
 * no isolated-workspace evidence (legacy shared checkout / absent line) — a
 * run without this evidence cannot prove it verified the candidate.
 */
export function verifiedShaFromWorkspaceEvidence(
  notes: string | null | undefined,
): string | null {
  const match = (notes ?? '').match(
    /Execution workspace:[^\n]*base=[^\s@]+@([0-9a-f]{40})/,
  )
  return match ? match[1]!.toLowerCase() : null
}

/** Explicit evidence a clean Assay appends: WHICH candidate SHA it verified. */
export function candidateVerifiedEvidenceLine(candidateSha: string): string {
  return `Assay verified candidate ${candidateSha} (workspace base == candidate).`
}

/** Factual Hold evidence when an Assay cannot be treated as a clean pass. */
export function assayHoldEvidenceLine(input: {
  candidateSha: string | null
  verifiedSha: string | null
  cleanEvidence: boolean
}): string {
  const candidate = input.candidateSha ? input.candidateSha.slice(0, 12) : '(none)'
  const verified = input.verifiedSha ? input.verifiedSha.slice(0, 12) : '(none)'
  if (!input.cleanEvidence) {
    return `Assay Hold: verification evidence reports a failed/missing Assay command or unresolved candidate; never normalized to Complete.`
  }
  if (!input.candidateSha) {
    return `Assay Hold: no Smith candidate commit exists for this story; Assay did not (and must not) verify a fallback base such as main.`
  }
  if (!input.verifiedSha) {
    return `Assay Hold: no workspace base evidence was recorded; the Assay cannot be proven to have run against candidate ${candidate}.`
  }
  return `Assay Hold: verification evidence is about workspace base ${verified}, which does not equal Smith candidate ${candidate}; the Assay did not verify the candidate commit.`
}

/**
 * Resolve the required workspace base ref for a claimed lane. An Assay/
 * verification lane must execute against the EXACT Smith candidate commit —
 * never the current `main`. A missing/unresolvable candidate fails closed with
 * a factual reason instead of silently falling back to `main`.
 */
export function resolveAssayWorkspaceBase(input: {
  role: string | null | undefined
  candidateSha: string | null
  fallbackBaseRef: string
}): { baseRef: string } | { error: string } {
  if (!isAssayTerminalRole(input.role)) {
    return { baseRef: input.fallbackBaseRef }
  }
  const candidate = commitSha(input.candidateSha)
  if (!candidate) {
    return {
      error:
        'Assay lane for a story with no resolvable Smith candidate commit; refusing to provision from main — a verification workspace must be based on the exact candidate commit (ENG-FORGE-V4-10C).',
    }
  }
  return { baseRef: candidate }
}
