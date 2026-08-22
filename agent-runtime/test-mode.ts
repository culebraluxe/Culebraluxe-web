// ---------------------------------------------------------------------------
// Runtime test execution mode (ENG-20A hot patch) — machine-visible verification
// scope policy.
//
//   SCOPED  (default)  targeted story tests + directly adjacent tests;
//                      typecheck/build only when warranted by touched code;
//                      MUST NOT launch the known full unit/persistence harness
//   FULL                full regression explicitly permitted (release gates,
//                      nightly validation, explicitly authorized infrastructure)
//   NONE                genuinely non-code / narrow verification operations
//
// AUTHORITY: the runtime test mode OUTRANKS contradictory story prose (goal /
// architect_brief / acceptance criteria / notes / special instructions). If
// SCOPED and the prose says "full suite", the runtime must tell the model that
// FULL is NOT authorized and that the runtime policy controls verification.
//
// PERSISTENCE: no schema change in this hot patch. The mode is carried inside
// the existing durable special_instructions envelope using a reserved directive
// line ("[runtime test-mode: SCOPED]") that the runtime parses and strips before
// the model sees the instructions. Future persistence (a dedicated test_mode
// column) can supersede this without changing the parse contract.
// ---------------------------------------------------------------------------

export type TestMode = 'SCOPED' | 'FULL' | 'NONE'

export const TEST_MODES: readonly TestMode[] = ['SCOPED', 'FULL', 'NONE']

export const DEFAULT_TEST_MODE: TestMode = 'SCOPED'

/** Reserved directive line embedded in special_instructions (machine-visible). */
export const TEST_MODE_DIRECTIVE_PREFIX = '[runtime test-mode: '

/** Parse a raw token into a TestMode, or null when unrecognized. */
export function parseTestMode(raw: string | null | undefined): TestMode | null {
  if (!raw) return null
  const upper = raw.trim().toUpperCase()
  if ((TEST_MODES as readonly string[]).includes(upper)) return upper as TestMode
  return null
}

/**
 * Extract the test-mode directive from the durable special_instructions.
 * Returns the mode (default SCOPED) and the instructions WITH the directive
 * removed, so the model never sees the directive token itself.
 */
export function resolveTestModeFromInstructions(
  specialInstructions: string | null,
  envOverride?: string | null,
): { mode: TestMode; instructions: string | null } {
  let mode: TestMode | null = null
  let instructions = specialInstructions ?? null

  if (instructions) {
    const m = instructions.match(/\[runtime\s+test-mode:\s*([^\]]+)\]/i)
    if (m) {
      mode = parseTestMode(m[1])
      instructions = instructions.replace(/\[runtime\s+test-mode:\s*[^\]]+\]\s*/i, '').trim() || null
    }
  }

  if (!mode && envOverride) {
    mode = parseTestMode(envOverride)
  }

  return { mode: mode ?? DEFAULT_TEST_MODE, instructions }
}

/** Wrap operator instructions with an explicit test-mode directive. */
export function withTestModeDirective(
  instructions: string,
  mode: TestMode,
): string {
  return `[runtime test-mode: ${mode}] ${instructions}`.trim()
}

// ---------------------------------------------------------------------------
// FULL harness guard (SCOPED protection)
// ---------------------------------------------------------------------------

/** Known full-regression command aliases discovered in package.json/scripts. */
export const FULL_REGRESSION_PATTERNS: RegExp[] = [
  /\bpnpm\s+test\b/,
  /\bpnpm\s+run\s+test\b/,
  /\bnpm\s+(run\s+)?test\b/,
  /\byarn\s+test\b/,
  /\bpnpm\s+test:persistence\b/,
  /\bpnpm\s+run\s+test:persistence\b/,
  /\bpnpm\s+test:engine\b/,
  /\bpnpm\s+run\s+test:engine\b/,
  /\bpnpm\s+test:app\b/,
  /\bpnpm\s+run\s+test:app\b/,
  /tsx\s+--test\s+workflow_engine\/tests\/\*\.test\.ts/,
  /tsx\s+--test\s+workflow_app\/tests\/\*\.test\.ts/,
  /node\s+--import\s+tsx\s+--test-concurrency=[0-9]+\s+--test\s+workflow_engine\/tests\/persistence/,
]

/** Return the matched forbidden command when `text` mentions a FULL invocation. */
export function detectFullRegressionAttempt(text: string | null | undefined): string | null {
  if (!text) return null
  for (const pattern of FULL_REGRESSION_PATTERNS) {
    const m = text.match(pattern)
    if (m) return m[0]
  }
  return null
}

/**
 * The authoritative, machine-visible verification policy injected into the
 * model's task text. In SCOPED mode this EXPLICITLY overrides contradictory
 * story prose ("full suite + tsc + build") and lists the forbidden commands.
 */
export function testModeTaskPolicy(mode: TestMode): string {
  if (mode === 'FULL') {
    return [
      'RUNTIME TEST EXECUTION POLICY: FULL — full regression is explicitly authorized by the runtime for this command.',
      'You may run the complete unit/persistence regression harness when the story warrants it, and you must report the results.',
    ].join('\n')
  }
  if (mode === 'NONE') {
    return [
      'RUNTIME TEST EXECUTION POLICY: NONE — no test execution is authorized for this command.',
      'Do NOT run the test suite, typecheck, or build. This is a narrow non-code verification operation.',
    ].join('\n')
  }
  return [
    'RUNTIME TEST EXECUTION POLICY: SCOPED (the runtime execution policy controls verification scope and OUTRANKS any story prose that asks for more).',
    'Even if the story goal / architect brief / acceptance criteria / notes / special instructions mention "full suite", "pnpm test", "pnpm test:persistence", or similar, FULL regression is NOT authorized for this command.',
    'Verify ONLY with targeted tests for the changed seams and directly adjacent tests; run typecheck/build only when the touched code warrants it.',
    'The following commands are FORBIDDEN and violate the runtime policy if invoked: pnpm test, pnpm test:persistence, pnpm test:engine, pnpm test:app, npm/yarn test, and any workflow_engine/workflow_app multi-file test glob.',
    'If a scoped verification cannot proceed, record the blocker and escalate; do NOT fall back to the full harness.',
  ].join('\n')
}

