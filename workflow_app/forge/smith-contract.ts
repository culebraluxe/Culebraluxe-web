// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-06 — typed Smith node execution contracts.
//
// Every Smith node runs against a bounded typed contract so a worker has
// explicit authority, inputs, outputs, and evidence obligations — whether it
// runs serially or inside a SPLIT. Malformed/incomplete contracts fail BEFORE
// worker launch. Contract scope defines mutation authority: Smith may not
// silently expand its own scope.
//
// Pure + DB-free.
// ---------------------------------------------------------------------------

export type SmithExecutionContract = {
  identity: { storyId: string; nodeId: string; attempt: number; owner: string }
  objective: string
  requiredInputs: string[]
  /** Permitted file-path areas (prefix semantics). */
  allowedScope: string[]
  /** Denied areas; always win over allowedScope. */
  prohibitedScope: string[]
  expectedOutputs: string[]
  /** Required targeted checks/commands the worker must run and report. */
  requiredEvidence: string[]
  /** Predecessor node ids that must be complete before this node runs. */
  dependsOn: string[]
}

export function validateSmithContract(c: SmithExecutionContract): { valid: boolean; errors: string[] } {
  const errors: string[] = []
  if (!c.identity.storyId) errors.push('missing storyId')
  if (!c.identity.nodeId) errors.push('missing nodeId')
  if (!c.identity.owner) errors.push('missing owner')
  if (!Number.isInteger(c.identity.attempt) || c.identity.attempt < 1) errors.push('attempt must be a positive integer')
  if (!c.objective?.trim()) errors.push('missing objective')
  if (!Array.isArray(c.allowedScope) || c.allowedScope.length === 0) errors.push('allowedScope must be non-empty')
  if (!Array.isArray(c.expectedOutputs) || c.expectedOutputs.length === 0) errors.push('expectedOutputs must be non-empty')
  if (!Array.isArray(c.requiredEvidence) || c.requiredEvidence.length === 0) errors.push('requiredEvidence must be non-empty')
  return { valid: errors.length === 0, errors }
}

export type LaunchReadiness = {
  launchable: boolean
  errors: string[]
}

/** Fail before launch when required inputs are absent or predecessors pending. */
export function canLaunch(
  c: SmithExecutionContract,
  state: { inputsPresent: string[]; predecessorsComplete: string[] },
): LaunchReadiness {
  const errors: string[] = []
  for (const input of c.requiredInputs) {
    if (!state.inputsPresent.includes(input)) errors.push(`missing required input: ${input}`)
  }
  for (const dep of c.dependsOn) {
    if (!state.predecessorsComplete.includes(dep)) errors.push(`dependency not complete: ${dep}`)
  }
  return { launchable: errors.length === 0, errors }
}

/**
 * Mutation authority: a changed path is allowed iff it matches an allowedScope
 * prefix and is NOT within any prohibitedScope prefix. Smith cannot expand scope.
 */
export function isChangeAllowed(c: SmithExecutionContract, changedPath: string): boolean {
  const prohibited = c.prohibitedScope.some((p) => pathWithin(changedPath, p))
  if (prohibited) return false
  return c.allowedScope.some((a) => pathWithin(changedPath, a))
}

function pathWithin(path: string, area: string): boolean {
  const p = path.replace(/\/+$/g, '')
  const a = area.replace(/\/+$/g, '')
  return p === a || p.startsWith(`${a}/`) || a === '*'
}
