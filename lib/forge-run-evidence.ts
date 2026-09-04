// Generic machine facts for one storyboard_story_run.
//
// This is intentionally lane-neutral. Smith, Assay, Scout, Inspector and
// future Forge lanes share the same persistence shape; fields stay null when a
// runtime cannot factually produce that measurement.

export type RunMachineEvidence = {
  baseCommitHash: string | null
  commandsTotal: number | null
  commandsPassed: number | null
  commandsFailed: number | null
  testsTotal: number | null
  testsPassed: number | null
  testsFailed: number | null
  policyViolationCount: number | null
  failureCode: string | null
  /** Large plain-text detail/evidence dumping ground; never parsed for truth. */
  evidenceDetail: string | null
}

export function hasStructuredRunMachineEvidence(
  evidence: RunMachineEvidence | null | undefined,
): boolean {
  if (!evidence) return false
  return (
    evidence.baseCommitHash !== null ||
    evidence.commandsTotal !== null ||
    evidence.commandsPassed !== null ||
    evidence.commandsFailed !== null ||
    evidence.testsTotal !== null ||
    evidence.testsPassed !== null ||
    evidence.testsFailed !== null ||
    evidence.policyViolationCount !== null ||
    evidence.failureCode !== null
  )
}

/**
 * Generic arithmetic cleanliness check. This intentionally does not impose a
 * lane-specific requirement such as "Assay must have at least one command";
 * consumers add those invariants on top of this common contract.
 */
export function isCleanRunMachineEvidence(
  evidence: RunMachineEvidence | null | undefined,
): boolean {
  if (!hasStructuredRunMachineEvidence(evidence) || !evidence) return false
  if (evidence.failureCode) return false
  if ((evidence.commandsFailed ?? 0) !== 0) return false
  if ((evidence.testsFailed ?? 0) !== 0) return false
  if ((evidence.policyViolationCount ?? 0) !== 0) return false
  if (
    evidence.commandsTotal !== null &&
    evidence.commandsPassed !== null &&
    evidence.commandsPassed !== evidence.commandsTotal
  ) {
    return false
  }
  if (
    evidence.testsTotal !== null &&
    evidence.testsPassed !== null &&
    evidence.testsPassed !== evidence.testsTotal
  ) {
    return false
  }
  return true
}
