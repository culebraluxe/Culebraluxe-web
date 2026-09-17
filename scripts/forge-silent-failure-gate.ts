// ---------------------------------------------------------------------------
// SILENT-FAILURE GATE — the Error Capture Obligation as a BLOCKING check.
//
// The learn loop already reports a swallowed catch as a nightly note. A note is not a gate: on
// 2026-09-16 six routes returned an empty success out of a bare catch, and an empty list reads as
// "this client has nothing" — indistinguishable from the truth and never reported as an outage. So the
// same detector now has an entry point that exits non-zero and names the file and line for a NEW
// empty-success catch on a server surface.
//
// It is DIFF-SCOPED on purpose. A repo walk would find the pre-existing hits and force an exemption
// list, which is the same lie moved to a new file. A hit on an added line is the author's; anything
// else is reported, not blocked.
//
// Test files are out of scope: a fixture that asserts "this IS a swallowed catch" contains one. The
// learn loop excludes them for the same reason.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'

import {
  findSilentFailures,
  isServerSurface,
  newSilentFailures,
  type SilentFailureHit,
} from '../agent-runtime/silent-failure-patterns'

export type GateFile = { path: string; content: string }

export type GateResult = {
  failures: SilentFailureHit[]
  output: string
  exitCode: 0 | 1
}

const CODE_EXTENSIONS = /\.(ts|tsx|mjs|js)$/
const SKIP_FILES = [/\.(test|spec)\.(ts|tsx|mjs|js)$/]

/** Would the gate judge this path? A handler lives on a server surface; fixtures and scripts do not. */
export function isGateSurface(path: string): boolean {
  if (!CODE_EXTENSIONS.test(path)) return false
  if (SKIP_FILES.some((pattern) => pattern.test(path))) return false
  return isServerSurface(path)
}

/**
 * The gate decision, pure over the files handed over.
 *
 * Only `empty-success-in-catch` blocks: the four report-only patterns are the learn loop's job, and a
 * shape that blocks must not also be queued as a story beside the block.
 */
export function evaluateGate(
  files: readonly GateFile[],
  addedLines: ReadonlyMap<string, ReadonlySet<number>>,
): GateResult {
  const scoped = files.filter((file) => isGateSurface(file.path))
  const failures = newSilentFailures(findSilentFailures(scoped), addedLines).filter(
    (hit) => hit.pattern === 'empty-success-in-catch',
  )
  const output = failures.map((hit) => `${hit.path}:${hit.line} [${hit.pattern}] ${hit.snippet}`).join('\n')
  return { failures, output, exitCode: failures.length > 0 ? 1 : 0 }
}

/** Added line numbers per file, parsed from a `git diff -U0` hunk header. */
export function addedLinesFromDiff(diff: string): Map<string, Set<number>> {
  const byFile = new Map<string, Set<number>>()
  let current: string | null = null
  for (const line of diff.split('\n')) {
    const fileMatch = /^\+\+\+ b\/(.+)$/.exec(line)
    if (fileMatch) {
      current = fileMatch[1]
      continue
    }
    const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(line)
    if (!hunk || !current) continue
    const start = Number.parseInt(hunk[1], 10)
    const count = hunk[2] === undefined ? 1 : Number.parseInt(hunk[2], 10)
    let set = byFile.get(current)
    if (!set) {
      set = new Set<number>()
      byFile.set(current, set)
    }
    for (let i = 0; i < count; i += 1) set.add(start + i)
  }
  return byFile
}

function git(args: string[], cwd: string): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
}

function main(): void {
  const cwd = process.cwd()
  const baseFlag = process.argv.indexOf('--base')
  const base = baseFlag >= 0 ? process.argv[baseFlag + 1] : process.env.GATE_BASE ?? 'origin/main'

  let mergeBase = base
  try {
    mergeBase = git(['merge-base', base, 'HEAD'], cwd).trim() || base
  } catch {
    mergeBase = base
  }

  let names = ''
  try {
    names = git(['diff', '--name-only', '--diff-filter=ACMR', mergeBase], cwd)
  } catch {
    names = ''
  }
  const paths = names
    .split('\n')
    .map((path) => path.trim())
    .filter((path) => path && isGateSurface(path))

  const files: GateFile[] = []
  for (const path of paths) {
    const full = `${cwd}/${path}`
    if (!existsSync(full)) continue
    files.push({ path, content: readFileSync(full, 'utf8') })
  }

  let addedLines = new Map<string, Set<number>>()
  if (paths.length > 0) {
    try {
      addedLines = addedLinesFromDiff(git(['diff', '-U0', mergeBase, '--', ...paths], cwd))
    } catch {
      addedLines = new Map()
    }
  }

  const { failures, output, exitCode } = evaluateGate(files, addedLines)
  if (failures.length > 0) {
    process.stderr.write(
      `silent-failure gate: ${failures.length} new empty-success catch(es) out of a bare catch\n${output}\n`,
    )
  } else {
    process.stdout.write('silent-failure gate: clean\n')
  }
  process.exitCode = exitCode
}

const invokedDirectly = Boolean(process.argv[1]?.endsWith('forge-silent-failure-gate.ts'))
if (invokedDirectly) main()
