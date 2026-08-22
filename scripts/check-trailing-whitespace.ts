// ---------------------------------------------------------------------------
// ENG-19-DOGFOOD-001 — repo invariant: no trailing whitespace in committed
// text files under the fixed allowlist.
//
//   pnpm exec tsx scripts/check-trailing-whitespace.ts
//
// Deterministic by construction:
//   - the file set comes from `git ls-files` (committed files only), restricted
//     to ALLOWLIST_ROOTS — no reliance on the working-tree walk or gitignore;
//   - binary files are skipped via the NUL-byte heuristic, so only text files
//     are scanned;
//   - the scan is byte-level (no encoding assumptions): a line violates the
//     invariant iff its final byte is a space (0x20) or a tab (0x09), after
//     stripping a CRLF terminator if present.
//
// Prints one `path:line` per violation and exits 1 when any exist; exits 0
// when the tree is clean. Importable for the invariant test
// (workflow_app/tests/trailing-whitespace-invariant.test.ts).
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

/** Fixed allowlist of committed directories the invariant covers. */
export const ALLOWLIST_ROOTS = [
  'agent-runtime',
  'db',
  'docs/workflow',
  'workflow_app',
  'workflow_engine',
] as const

/** One trailing-whitespace finding: `file` is repo-relative, `line` is 1-based. */
export interface Violation {
  file: string
  line: number
}

export interface CheckResult {
  violations: Violation[]
  filesScanned: number
}

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(SCRIPT_DIR, '..')

/** Committed files under `roots`, listed by git so the scan is deterministic. */
export function listCommittedFiles(
  roots: readonly string[],
  cwd: string = REPO_ROOT,
): string[] {
  const out = execFileSync(
    'git',
    ['ls-files', '-z', '--', ...roots],
    { cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  )
  return out.split('\0').filter((file) => file.length > 0)
}

/** NUL-byte heuristic: files containing a NUL in the first 8 KiB are binary. */
export function isBinary(buffer: Buffer): boolean {
  return buffer.subarray(0, 8192).includes(0)
}

/**
 * Byte-level scan: 1-based line numbers whose final byte is a space or tab
 * (a CRLF terminator, if present, is stripped before the check).
 */
export function scanForTrailingWhitespace(buffer: Buffer): number[] {
  const badLines: number[] = []
  let lineStart = 0
  let lineNo = 1
  const checkLine = (lineEnd: number): void => {
    if (lineEnd > lineStart && buffer[lineEnd - 1] === 0x0d) lineEnd -= 1
    const last = lineEnd > lineStart ? buffer[lineEnd - 1] : 0
    if (last === 0x20 || last === 0x09) badLines.push(lineNo)
  }
  for (let i = 0; i < buffer.length; i += 1) {
    if (buffer[i] === 0x0a) {
      checkLine(i)
      lineStart = i + 1
      lineNo += 1
    }
  }
  if (lineStart < buffer.length) checkLine(buffer.length)
  return badLines
}

/** Run the invariant check against the repo. Deterministic; no side effects. */
export function checkTrailingWhitespace(
  options: { roots?: readonly string[]; cwd?: string } = {},
): CheckResult {
  const roots = options.roots ?? ALLOWLIST_ROOTS
  const cwd = options.cwd ?? REPO_ROOT
  const violations: Violation[] = []
  let filesScanned = 0
  for (const file of listCommittedFiles(roots, cwd)) {
    const buffer = readFileSync(resolve(cwd, file))
    if (isBinary(buffer)) continue
    filesScanned += 1
    for (const line of scanForTrailingWhitespace(buffer)) {
      violations.push({ file, line })
    }
  }
  return { violations, filesScanned }
}

/** Render a finding as `path:line`. */
export function formatViolation(violation: Violation): string {
  return `${violation.file}:${violation.line}`
}

function main(): void {
  const result = checkTrailingWhitespace()
  for (const violation of result.violations) {
    console.log(formatViolation(violation))
  }
  if (result.violations.length > 0) {
    console.error(
      `trailing whitespace: ${result.violations.length} violation(s) in ` +
        `${result.filesScanned} text file(s) — fix and re-run`,
    )
    process.exit(1)
  }
  console.log(`trailing whitespace: ok (${result.filesScanned} file(s) scanned)`)
  process.exit(0)
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main()
}
