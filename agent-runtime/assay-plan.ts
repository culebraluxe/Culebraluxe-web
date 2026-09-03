import {
  DEFAULT_TEST_MODE,
  detectFullRegressionAttempt,
  parseTestMode,
  type TestMode,
  withTestModeDirective,
} from './test-mode'

export type AssayPlan =
  | { ok: true; mode: TestMode; commands: string[]; instructions: string }
  | { ok: false; code: 'missing-assay-plan' | 'full-not-authorized'; reason: string }

const COMMAND_LINE = /^\s*(?:[-*]\s+)?(`?)(.+?)\1\s*$/

export function parseAssayCommands(section: string | null | undefined): string[] {
  if (!section) return []
  return section
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
    .map((line) => {
      const m = line.match(COMMAND_LINE)
      return (m?.[2] ?? line).trim()
    })
    .filter((line) => line.length > 0)
}

export function planAssay(input: {
  testMode?: string | null
  assayCommands?: string | null
}): AssayPlan {
  const mode = parseTestMode(input.testMode) ?? DEFAULT_TEST_MODE
  const commands = parseAssayCommands(input.assayCommands)
  if (commands.length === 0) {
    return {
      ok: false,
      code: 'missing-assay-plan',
      reason: 'Assay needs ## Assay commands on the packet. Do not invent pnpm test.',
    }
  }
  if (mode !== 'FULL') {
    const banned = commands.map(detectFullRegressionAttempt).find(Boolean)
    if (banned) {
      return {
        ok: false,
        code: 'full-not-authorized',
        reason: `Assay command '${banned}' is FULL regression; packet Test mode is ${mode}`,
      }
    }
  }
  const list = commands.map((c) => `- ${c}`).join('\n')
  const instructions = [
    `Assay plan (${mode}). Run ONLY these commands in the Smith worktree.`,
    'Do not edit files, commit, migrate, or mark the story Complete.',
    'Write the exact command + exit + tail to tests_summary.',
    'If a command fails, stop. Inventiveness is a defect.',
    list,
  ].join('\n')
  return { ok: true, mode, commands, instructions: withTestModeDirective(instructions, mode) }
}
