import type { ForgeFailureCode } from './forge-failure'

export const ASSAY_EVIDENCE_VERSION = 1 as const
const PLAN_DIRECTIVE = 'forge-assay-plan:v1:'
const CANDIDATE_DIRECTIVE = 'forge-assay-candidate:v1:'

export type AssayTestCounters = {
  total: number | null
  passed: number | null
  failed: number | null
}

export type AssayCommandResult = {
  command: string
  exitCode: number | null
  signal: string | null
  timedOut: boolean
  durationMs: number
  tests: AssayTestCounters
  stdoutTail: string
  stderrTail: string
}

export type AssayEvidence = {
  version: typeof ASSAY_EVIDENCE_VERSION
  candidateSha: string | null
  verifiedSha: string | null
  requiredCommands: string[]
  commandResults: AssayCommandResult[]
  policyViolations: string[]
  startedAt: string
  endedAt: string
  verdict: 'PASS' | 'FAIL'
  failureCode: ForgeFailureCode | null
  failureDetail: string | null
}

export type AssayVerdict = {
  pass: boolean
  failureCode: ForgeFailureCode | null
  detail: string | null
}

function normalizeSha(value: string | null | undefined): string | null {
  const sha = (value ?? '').trim().toLowerCase()
  return /^[0-9a-f]{40}$/.test(sha) ? sha : null
}

export function withAssayPlanDirective(
  instructions: string,
  input: { mode: string; commands: string[] },
): string {
  const payload = Buffer.from(JSON.stringify(input), 'utf8').toString('base64url')
  return `[${PLAN_DIRECTIVE}${payload}] ${instructions}`.trim()
}

export function assayPlanFromInstructions(
  instructions: string | null | undefined,
): { mode: string; commands: string[] } | null {
  const match = (instructions ?? '').match(
    /\[forge-assay-plan:v1:([A-Za-z0-9_-]+)\]/,
  )
  if (!match) return null
  try {
    const parsed = JSON.parse(
      Buffer.from(match[1], 'base64url').toString('utf8'),
    ) as { mode?: unknown; commands?: unknown }
    if (
      typeof parsed.mode !== 'string' ||
      !Array.isArray(parsed.commands) ||
      parsed.commands.some((command) => typeof command !== 'string')
    ) {
      return null
    }
    const commands = parsed.commands.map((command) => command.trim()).filter(Boolean)
    return { mode: parsed.mode, commands }
  } catch {
    return null
  }
}

export function withAssayCandidateDirective(
  instructions: string,
  candidateSha: string,
): string {
  const sha = normalizeSha(candidateSha)
  if (!sha) throw new Error(`invalid Assay candidate SHA: ${candidateSha}`)
  return `[${CANDIDATE_DIRECTIVE}${sha}] ${instructions}`.trim()
}

export function assayCandidateFromInstructions(
  instructions: string | null | undefined,
): string | null {
  const match = (instructions ?? '').match(
    /\[forge-assay-candidate:v1:([0-9a-fA-F]{40})\]/,
  )
  return normalizeSha(match?.[1] ?? null)
}

function lastCounter(text: string, patterns: RegExp[]): number | null {
  let value: number | null = null
  for (const pattern of patterns) {
    pattern.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = pattern.exec(text))) value = Number(match[1])
  }
  return value
}

/**
 * Parse numeric test-runner counters only. Labels locate counters; words do not
 * determine success. The verdict uses these numbers plus process exit status.
 */
export function parseAssayTestCounters(output: string): AssayTestCounters {
  const fractionMatches = Array.from(
    output.matchAll(/\b(\d+)\s*\/\s*(\d+)\s+(?:pass|passed)\b/gi),
  )
  const fraction = fractionMatches.at(-1) ?? null
  const fractionPassed = fraction ? Number(fraction[1]) : null
  const fractionTotal = fraction ? Number(fraction[2]) : null

  const total =
    lastCounter(output, [
      /(?:^|\n)\s*#?\s*tests?\s*[:=]?\s*(\d+)\b/gim,
      /\btests?\s*[:=]\s*(\d+)\b/gi,
    ]) ?? fractionTotal
  const passed =
    lastCounter(output, [
      /(?:^|\n)\s*#?\s*pass(?:ed)?\s*[:=]?\s*(\d+)\b/gim,
      /\bpass(?:ed)?\s*[:=]\s*(\d+)\b/gi,
      /\b(\d+)\s+pass(?:ed)?\b/gi,
    ]) ?? fractionPassed
  const failed = lastCounter(output, [
    /(?:^|\n)\s*#?\s*fail(?:ed|ures?)?\s*[:=]?\s*(\d+)\b/gim,
    /\bfail(?:ed|ures?)?\s*[:=]\s*(\d+)\b/gi,
    /\b(\d+)\s+fail(?:ed|ures?)?\b/gi,
    /(?:^|\n)\s*#?\s*errors?\s*[:=]?\s*(\d+)\b/gim,
  ])

  return { total, passed, failed }
}

/**
 * Pure arithmetic Assay verdict. No tests_summary, notes, model prose, or
 * sentiment scan can influence this function.
 */
export function evaluateAssayEvidence(
  evidence: Omit<AssayEvidence, 'verdict' | 'failureCode' | 'failureDetail'>,
): AssayVerdict {
  const candidate = normalizeSha(evidence.candidateSha)
  const verified = normalizeSha(evidence.verifiedSha)
  if (!candidate || !verified || candidate !== verified) {
    return {
      pass: false,
      failureCode: 'CANDIDATE_MISMATCH',
      detail:
        `Assay candidate mismatch: candidate=${candidate ?? '(none)'} ` +
        `verified=${verified ?? '(none)'}.`,
    }
  }

  if (evidence.requiredCommands.length === 0) {
    return {
      pass: false,
      failureCode: 'MISSING_ASSAY_PLAN',
      detail: 'Assay has no required commands.',
    }
  }

  if (evidence.policyViolations.length > 0) {
    return {
      pass: false,
      failureCode: 'ASSAY_POLICY_FAILED',
      detail: evidence.policyViolations.join(' | '),
    }
  }

  if (evidence.commandResults.length !== evidence.requiredCommands.length) {
    return {
      pass: false,
      failureCode: 'ASSAY_TEST_FAILED',
      detail:
        `Assay completed ${evidence.commandResults.length}/${evidence.requiredCommands.length} required commands.`,
    }
  }

  for (let i = 0; i < evidence.requiredCommands.length; i += 1) {
    const required = evidence.requiredCommands[i]
    const result = evidence.commandResults[i]
    if (!result || result.command !== required) {
      return {
        pass: false,
        failureCode: 'ASSAY_TEST_FAILED',
        detail: `Assay command ${i + 1} does not match the immutable plan.`,
      }
    }
    if (result.timedOut || result.exitCode !== 0) {
      return {
        pass: false,
        failureCode: 'ASSAY_TEST_FAILED',
        detail:
          `Assay command failed: ${required} ` +
          `(exit=${result.exitCode ?? 'none'}${result.timedOut ? ', timeout' : ''}).`,
      }
    }
    if (result.tests.failed !== null && result.tests.failed !== 0) {
      return {
        pass: false,
        failureCode: 'ASSAY_TEST_FAILED',
        detail: `Assay command reported ${result.tests.failed} failed tests: ${required}`,
      }
    }
    if (
      result.tests.total !== null &&
      result.tests.passed !== null &&
      result.tests.total !== result.tests.passed
    ) {
      return {
        pass: false,
        failureCode: 'ASSAY_TEST_FAILED',
        detail:
          `Assay command passed ${result.tests.passed}/${result.tests.total} tests: ${required}`,
      }
    }
  }

  return { pass: true, failureCode: null, detail: null }
}

export function finalizeAssayEvidence(
  evidence: Omit<AssayEvidence, 'verdict' | 'failureCode' | 'failureDetail'>,
): AssayEvidence {
  const verdict = evaluateAssayEvidence(evidence)
  return {
    ...evidence,
    verdict: verdict.pass ? 'PASS' : 'FAIL',
    failureCode: verdict.failureCode,
    failureDetail: verdict.detail,
  }
}

export function assayEvidenceSummary(evidence: AssayEvidence): string {
  const commandSummary = evidence.commandResults
    .map((result) => {
      const counts =
        result.tests.total !== null ||
        result.tests.passed !== null ||
        result.tests.failed !== null
          ? ` tests=${result.tests.total ?? '?'} pass=${result.tests.passed ?? '?'} fail=${result.tests.failed ?? '?'}`
          : ''
      return `${result.command} -> exit ${result.exitCode ?? 'none'}${counts}`
    })
    .join(' | ')
  return [
    `Assay ${evidence.verdict}`,
    `candidate=${evidence.candidateSha ?? '(none)'}`,
    `verified=${evidence.verifiedSha ?? '(none)'}`,
    commandSummary,
    evidence.failureDetail ?? '',
  ]
    .filter(Boolean)
    .join(' | ')
}
