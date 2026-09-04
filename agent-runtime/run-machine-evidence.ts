import type { RunMachineEvidence } from '../lib/forge-run-evidence'
import { parseAssayArithmeticFacts } from './assay-arithmetic'
import type { AssayCommandResult, AssayEvidence } from './assay-evidence'
import { verifiedShaFromWorkspaceEvidence } from './candidate-assay-handoff'

function commandPassed(result: AssayCommandResult): boolean {
  if (result.timedOut || result.exitCode !== 0) return false
  if (result.tests.failed !== null && result.tests.failed !== 0) return false
  if (
    result.tests.total !== null &&
    result.tests.passed !== null &&
    result.tests.total !== result.tests.passed
  ) {
    return false
  }
  return true
}

function sumKnown(
  values: Array<number | null>,
): number | null {
  const known = values.filter((value): value is number => value !== null)
  return known.length > 0 ? known.reduce((sum, value) => sum + value, 0) : null
}

function assayDetail(evidence: AssayEvidence): string {
  const commands = evidence.commandResults.map((result, index) => {
    const lines = [
      `Command ${index + 1}: ${result.command}`,
      `exit=${result.exitCode ?? 'none'} timeout=${result.timedOut ? 'yes' : 'no'} duration_ms=${result.durationMs}`,
      `tests total=${result.tests.total ?? '?'} passed=${result.tests.passed ?? '?'} failed=${result.tests.failed ?? '?'}`,
    ]
    if (result.stdoutTail.trim()) lines.push(`stdout:\n${result.stdoutTail.trim()}`)
    if (result.stderrTail.trim()) lines.push(`stderr:\n${result.stderrTail.trim()}`)
    return lines.join('\n')
  })

  return [
    `Assay verdict: ${evidence.verdict}`,
    `candidate=${evidence.candidateSha ?? '(none)'}`,
    `verified=${evidence.verifiedSha ?? '(none)'}`,
    evidence.failureDetail ? `failure: ${evidence.failureDetail}` : null,
    evidence.policyViolations.length
      ? `policy violations:\n${evidence.policyViolations.map((v) => `- ${v}`).join('\n')}`
      : null,
    ...commands,
  ]
    .filter(Boolean)
    .join('\n\n')
}

/**
 * Convert any lane finish into the common storyboard_story_run evidence shape.
 * Assay has richer structured input; ordinary model/runtime lanes use the
 * numeric facts already present in their tests summary and workspace evidence.
 */
export function runMachineEvidenceFromFinish(input: {
  role: string | null | undefined
  resultStatus: string
  notes: string
  testsSummary: string | null
  assayEvidence?: AssayEvidence | null
}): RunMachineEvidence {
  const assay = input.assayEvidence ?? null
  if (assay) {
    const commandsPassed = assay.commandResults.filter(commandPassed).length
    const commandsTotal = assay.requiredCommands.length
    return {
      baseCommitHash:
        assay.verifiedSha ?? verifiedShaFromWorkspaceEvidence(input.notes),
      commandsTotal,
      commandsPassed,
      commandsFailed: Math.max(0, commandsTotal - commandsPassed),
      testsTotal: sumKnown(assay.commandResults.map((r) => r.tests.total)),
      testsPassed: sumKnown(assay.commandResults.map((r) => r.tests.passed)),
      testsFailed: sumKnown(assay.commandResults.map((r) => r.tests.failed)),
      policyViolationCount: assay.policyViolations.length,
      failureCode: assay.failureCode,
      evidenceDetail: assayDetail(assay),
    }
  }

  const facts = parseAssayArithmeticFacts(input.testsSummary)
  const complete = /^complete$/i.test(input.resultStatus.trim())
  const role = (input.role ?? '').trim().toLowerCase()
  const policyViolationCount = facts.policyViolations
  const failureCode = policyViolationCount > 0
    ? 'EXECUTION_CONTRACT_FAILED'
    : complete
      ? null
      : role === 'builder'
        ? 'SMITH_RESULT_FAILED'
        : 'HUMAN_DECISION_REQUIRED'

  return {
    baseCommitHash: verifiedShaFromWorkspaceEvidence(input.notes),
    commandsTotal: facts.exitCodes.length > 0 ? facts.exitCodes.length : null,
    commandsPassed:
      facts.exitCodes.length > 0
        ? facts.exitCodes.filter((code) => code === 0).length
        : null,
    commandsFailed:
      facts.exitCodes.length > 0
        ? facts.exitCodes.filter((code) => code !== 0).length
        : null,
    testsTotal: facts.testsTotal,
    testsPassed: facts.testsPassed,
    testsFailed: facts.testsFailed,
    policyViolationCount,
    failureCode,
    evidenceDetail: input.notes.trim() || null,
  }
}
