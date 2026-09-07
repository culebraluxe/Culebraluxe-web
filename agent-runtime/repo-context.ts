import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import type { StoryRun } from '../db/storyboard'

const DEFAULT_MAX_OUTPUT_CHARS = 32_000
const DEFAULT_TIMEOUT_MS = 20_000
const SCOUT_RESEARCH_MAX_CHARS = 8_000

type ExecFileLike = (
  file: string,
  args: readonly string[],
  options: {
    cwd: string
    encoding: 'utf8'
    timeout: number
    maxBuffer: number
  },
) => string

export function resolveRipwireBin(
  env: NodeJS.ProcessEnv = process.env,
  home = homedir(),
): string {
  const explicit = (env.RIPWIRE_BIN ?? '').trim()
  if (explicit) return explicit

  // launchd/non-interactive workers do not necessarily source ~/.zshrc, so
  // prefer the standard Ripwire installer target before relying on PATH.
  const localInstall = join(home, '.local', 'bin', 'ripwire')
  return existsSync(localInstall) ? localInstall : 'ripwire'
}

export function buildRepoContextQuery(input: {
  id: string
  title: string
  goal?: string | null
  scope?: string | null
  architectBrief?: string | null
}): string {
  return [
    `${input.id}: ${input.title}`,
    input.goal ? `Goal: ${input.goal}` : null,
    input.scope ? `Scope: ${input.scope}` : null,
    input.architectBrief ? `Architect brief: ${input.architectBrief}` : null,
  ]
    .filter(Boolean)
    .join('\n')
    .slice(0, 4_000)
}

/**
 * Read-only repository intelligence. Failure is deliberately non-fatal: Scout
 * still runs when Ripwire is absent or a repository cannot be indexed.
 */
export function runRepoContextTaskPacket(input: {
  workspace: string
  task: string
  bin?: string
  maxOutputChars?: number
  timeoutMs?: number
  execFile?: ExecFileLike
}): string | null {
  const task = input.task.trim()
  if (!task) return null

  const execFile = input.execFile ?? (execFileSync as unknown as ExecFileLike)
  const maxOutputChars = input.maxOutputChars ?? DEFAULT_MAX_OUTPUT_CHARS
  try {
    const output = execFile(
      input.bin ?? resolveRipwireBin(),
      [
        '.',
        '--exclude=.next',
        '--exclude=node_modules',
        '--exclude=.ripwire-output',
        `--pack-task=${task}`,
      ],
      {
        cwd: input.workspace,
        encoding: 'utf8',
        timeout: input.timeoutMs ?? DEFAULT_TIMEOUT_MS,
        maxBuffer: 2 * 1024 * 1024,
      },
    ).trim()
    if (!output) return null
    if (output.length <= maxOutputChars) return output
    return `${output.slice(0, maxOutputChars)}\n<!-- Forge truncated Ripwire output at ${maxOutputChars} chars -->`
  } catch {
    return null
  }
}

export function withRepoContextPacket(
  instructions: string | null | undefined,
  packet: string | null | undefined,
): string | null {
  const base = (instructions ?? '').trim()
  const context = (packet ?? '').trim()
  if (!context) return base || null

  return [
    base || null,
    'Repository context (Ripwire structural evidence; use it to orient, then verify important conclusions in source):',
    context,
    'SCOUT RESEARCH CONTRACT: end your final report with a section beginning exactly "SCOUT_RESEARCH:". Keep it concise and factual. Include the likely owning symbols/files, important callers/blast radius, relevant tests, uncertainties, and your recommended next lane/action. This final section is durable handoff evidence and will be persisted to the Story Run in Neon.',
  ]
    .filter(Boolean)
    .join('\n\n')
}

/** Latest durable Scout synthesis for downstream lanes. */
export function latestScoutResearch(runs: readonly StoryRun[]): string | null {
  const run = runs.find(
    (item) => item.runType === 'scout' && (item.notes ?? '').trim().length > 0,
  )
  if (!run?.notes) return null

  const notes = run.notes.trim()
  const marker = 'SCOUT_RESEARCH:'
  const markerIndex = notes.lastIndexOf(marker)
  const research = markerIndex >= 0 ? notes.slice(markerIndex) : notes
  if (research.length <= SCOUT_RESEARCH_MAX_CHARS) return research
  return `${research.slice(0, SCOUT_RESEARCH_MAX_CHARS)}\n[Scout research truncated by Forge]`
}

export function withScoutResearch(
  instructions: string | null | undefined,
  research: string | null | undefined,
): string | null {
  const base = (instructions ?? '').trim()
  const evidence = (research ?? '').trim()
  if (!evidence) return base || null
  return [
    base || null,
    'Prior Scout research from the durable Story Run (evidence, not authority):',
    evidence,
  ]
    .filter(Boolean)
    .join('\n\n')
}
