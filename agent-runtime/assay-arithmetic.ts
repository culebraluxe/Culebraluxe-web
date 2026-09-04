// ---------------------------------------------------------------------------
// Assay arithmetic — numeric verification facts, never prose sentiment.
//
// Once a verifier summary contains machine-readable numeric evidence, the
// acceptance decision is arithmetic only:
//   - every reported command exit code must be 0
//   - reported failed/error count must be 0
//   - when total + passed are both reported, passed must equal total
//   - explicit runtime policy-violation count must be 0
//
// Human prose may still be retained for diagnostics, but words such as
// "failed commands" cannot override 26/26 pass, fail 0, exit 0.
// Legacy summaries with no numeric facts fall back to the older defensive
// vocabulary in candidate-assay-handoff.ts until historical evidence ages out.
// ---------------------------------------------------------------------------

export type AssayArithmeticFacts = {
  exitCodes: number[]
  testsTotal: number | null
  testsPassed: number | null
  testsFailed: number | null
  policyViolations: number
  hasNumericEvidence: boolean
}

function lastNumber(text: string, patterns: RegExp[]): number | null {
  let value: number | null = null
  for (const pattern of patterns) {
    pattern.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = pattern.exec(text))) {
      value = Number(match[1])
    }
  }
  return value
}

/**
 * Parse only explicit numeric facts. This does NOT infer failure from words.
 * Labels such as "fail" and "error" are used solely to locate their numeric
 * counters; the counter value is what the verdict uses.
 */
export function parseAssayArithmeticFacts(
  testsSummary: string | null | undefined,
): AssayArithmeticFacts {
  const summary = testsSummary ?? ''

  const exitCodes: number[] = []
  for (const pattern of [
    /\bexit\s+(-?\d+)\b/gi,
    /\bexit\s+code\s*[:=]?\s*(-?\d+)\b/gi,
  ]) {
    let match: RegExpExecArray | null
    while ((match = pattern.exec(summary))) {
      exitCodes.push(Number(match[1]))
    }
  }

  const fractions = [
    ...summary.matchAll(/\b(\d+)\s*\/\s*(\d+)\s+(?:pass|passed)\b/gi),
  ]
  const fraction = fractions.at(-1) ?? null

  let testsPassed = fraction ? Number(fraction[1]) : null
  let testsTotal = fraction ? Number(fraction[2]) : null

  if (testsTotal === null) {
    testsTotal = lastNumber(summary, [/\btests?\s*[:=]?\s*(\d+)\b/gi])
  }
  if (testsPassed === null) {
    testsPassed = lastNumber(summary, [
      /\bpass(?:ed)?\s*[:=]?\s*(\d+)\b/gi,
      /\b(\d+)\s+pass(?:ed)?\b/gi,
    ])
  }

  const testsFailed = lastNumber(summary, [
    /\bfail(?:ed|ures?)?\s*[:=]?\s*(\d+)\b/gi,
    /\b(\d+)\s+fail(?:ed|ures?)?\b/gi,
    /\berrors?\s*[:=]?\s*(\d+)\b/gi,
    /\b(\d+)\s+errors?\b/gi,
  ])

  const policyViolations =
    (summary.match(/TEST-MODE VIOLATION/gi) ?? []).length +
    (summary.match(/POLICY VIOLATION/gi) ?? []).length

  const hasNumericEvidence =
    exitCodes.length > 0 ||
    testsTotal !== null ||
    testsPassed !== null ||
    testsFailed !== null

  return {
    exitCodes,
    testsTotal,
    testsPassed,
    testsFailed,
    policyViolations,
    hasNumericEvidence,
  }
}

/** Arithmetic verdict for summaries that contain numeric evidence. */
export function isArithmeticAssayPass(facts: AssayArithmeticFacts): boolean {
  if (!facts.hasNumericEvidence) return false
  if (facts.exitCodes.some((code) => code !== 0)) return false
  if (facts.testsFailed !== null && facts.testsFailed !== 0) return false
  if (
    facts.testsTotal !== null &&
    facts.testsPassed !== null &&
    facts.testsPassed !== facts.testsTotal
  ) {
    return false
  }
  if (facts.policyViolations !== 0) return false
  return true
}
