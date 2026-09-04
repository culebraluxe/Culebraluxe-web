import {
  DEFAULT_TEST_MODE,
  detectFullRegressionAttempt,
  parseTestMode,
  type TestMode,
  withTestModeDirective,
} from './test-mode'
import { withAssayPlanDirective } from './assay-evidence'

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
  const humanInstructions = [
    `Assay plan (${mode}). Forge executes ONLY these immutable commands in the exact Smith candidate worktree.`,
    'Assay is deterministic and model-free: do not edit files, commit, migrate, invent commands, or infer success from prose.',
    'Process exit codes and numeric test counters are the acceptance facts. A failed command stops the Assay and hands the story to a human.',
    list,
  ].join('\n')
  const runtimeInstructions = withTestModeDirective(humanInstructions, mode)
  return {
    ok: true,
    mode,
    commands,
    instructions: withAssayPlanDirective(runtimeInstructions, { mode, commands }),
  }
}
