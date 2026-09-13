/**
 * Injected-effect implementations for the role ports. These are the RUNNER's
 * side of the contract: the agent package never spawns a process itself.
 */
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import type { CommandResult, StaticSlice } from './ports'
import { runStaticGate } from '../forge-static-gate'
import type { StaticGateResult } from '../forge-static-gate'

const EXCERPT_CAP = 240
const DEFAULT_TIMEOUT_MS = 600_000

/**
 * Bounded, synchronous command execution for the Assay lane.
 *
 * The commands are the STORY'S FROZEN PROOFS (never invented here), run inside
 * the candidate's own worktree. The excerpt is capped because it is stored as
 * evidence, not read as a log.
 */
export function commandRunner(
  cwd: string,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): (command: string) => CommandResult {
  return (command) => {
    const result = spawnSync(command, {
      cwd,
      shell: true,
      encoding: 'utf8',
      timeout: timeoutMs,
      maxBuffer: 32 * 1024 * 1024,
    })
    const output = `${result.stdout ?? ''}${result.stderr ?? ''}`
    return {
      command,
      exitCode: result.status ?? 1,
      passed: result.status === 0,
      excerpt: output.replace(/\s+/g, ' ').trim().slice(0, EXCERPT_CAP),
    }
  }
}

/** Normalize the live static gate result to the port's narrower slice. */
export function staticSliceFromGate(result: StaticGateResult): StaticSlice {
  return {
    archRan: result.archRan,
    archOk: result.archOk,
    archErrors: result.archErrors,
    semgrepFindings: result.semgrepFindings,
    knipFindings: result.knipFindings,
  }
}

/**
 * Run the live static gate against a candidate WORKTREE, pointing at the primary
 * checkout's binaries.
 *
 * The worktree has the CODE but no `node_modules`, so without this depcruise finds
 * no tool, does not run, and "did not run" is not a failure — which makes the HARD
 * architecture gate decorative. `toolsRoot` is the primary checkout.
 *
 * Only pass a binary that EXISTS: passing a path that isn't there is what
 * false-failed this gate before (see scripts/forge-tools.ts).
 */
export function staticSliceForWorktree(workspace: string, toolsRoot: string): StaticSlice {
  const depcruiseBin = resolve(toolsRoot, 'node_modules/.bin/depcruise')
  const knipBin = resolve(toolsRoot, 'node_modules/.bin/knip')
  return staticSliceFromGate(
    runStaticGate({
      workspace,
      ...(existsSync(depcruiseBin) ? { depcruiseBin } : {}),
      ...(existsSync(knipBin) ? { knipBin } : {}),
    }),
  )
}
