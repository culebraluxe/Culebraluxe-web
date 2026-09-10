// ---------------------------------------------------------------------------
// ENG-FORGE-SPLIT-01 — what did this candidate actually change?
//
// A SPLIT child owns ONE accepted assignment with an `allowedScope` and a
// `prohibitedScope`. Until this existed, those were declared (contract + work-order
// prose) but never checked against the child's REAL diff, so a confused Smith could
// touch a sibling's file, present a SHA, and the join would pass because it only
// asked "did every child produce a SHA?".
//
// Diffing from the MERGE BASE (not from a moving ref) yields exactly the child's own
// changes relative to the point its branch was created.
//
// Fail-closed by design: a git failure throws. Callers must treat an unresolvable
// diff as a HOLD — never as "no violations".
// ---------------------------------------------------------------------------

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const run = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await run('git', args, { cwd, maxBuffer: 8 * 1024 * 1024 })
  return stdout
}

export async function changedFilesForCandidate(input: {
  cwd: string
  baseRef: string
  candidateSha: string
}): Promise<string[]> {
  const mergeBase = (await git(input.cwd, ['merge-base', input.baseRef, input.candidateSha])).trim()
  if (!mergeBase) throw new Error(`no merge base between ${input.baseRef} and ${input.candidateSha}`)
  const out = await git(input.cwd, ['diff', '--name-only', mergeBase, input.candidateSha])
  return out
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
}
