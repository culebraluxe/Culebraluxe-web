// ---------------------------------------------------------------------------
// CANDIDATE-OWN-DIFF SECRET SCAN (ENG-FORGE-LANE-SECRET-GATE-01, widened by
// FORGE-PUBLISH-SCAN-COVERAGE-01 on 2026-09-18).
//
// The gate reads only each commit's OWN changes: for each commit in the range, the added lines of
// `git diff <c>^ <c>`. It never scans the tree, so a credential that already exists in the BASE is never
// attributed to this lane.
//
// WHAT WIDENED, and why the caller changed: this module was always able to read several commits, but the
// publisher handed it `commits: [candidate]` — the tip — so a credential introduced in an earlier
// unpublished commit rode along behind a clean final commit (Astra review 1.3, reproduced). The caller now
// passes the whole range the push would send (lib/worker-workspace/publish-range.ts), which means a
// credential in a FOREIGN but still-unpublished commit inside that range is now attributed to whoever
// publishes it. That is the honest reading of "the range is being published": the base is excluded, the
// rest is not.
//
// The git read is injected, so the rule is testable without a repository.
//
// FAIL CLOSED: an unreadable diff is reported as `unreadable`, and the caller must refuse to publish. A
// scan that cannot run is never "clean".
// ---------------------------------------------------------------------------

import { secretShapesInLine } from '../secret-shapes'

export type CandidateSecretFinding = {
  /** The catalog rule that matched (e.g. `openai-style key`). */
  rule: string
  /** Repository-relative path the added line lives in. */
  file: string
  /** Line number in the new file. */
  line: number
}

export type CandidateAddedLine = {
  file: string
  line: number
  text: string
}

export type CandidateSecretScan = {
  findings: CandidateSecretFinding[]
  /** Commits whose diff could not be read. Non-empty means UNSCANNED, not clean. */
  unreadable: string[]
}

/**
 * Parse the ADDED (`+`) lines out of a unified diff, with their file and new-file line number.
 *
 * Pure: the diff text is the only input. A `+++`/`---` header is not an added line, and a deletion
 * (`-`) does not advance the new-file counter, so a matched line number points at the added text.
 */
export function parseAddedLines(diffText: string): CandidateAddedLine[] {
  const out: CandidateAddedLine[] = []
  let file: string | null = null
  let newLine = 0
  for (const raw of diffText.split('\n')) {
    if (raw.startsWith('diff --git ')) {
      file = null
      continue
    }
    if (raw.startsWith('+++ ')) {
      const target = raw.slice(4).trim()
      file = target === '/dev/null' ? null : target.replace(/^b\//, '')
      continue
    }
    const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(raw)
    if (hunk) {
      newLine = Number.parseInt(hunk[1], 10)
      continue
    }
    if (raw.startsWith('---')) continue
    if (raw.startsWith('+')) {
      if (file) out.push({ file, line: newLine, text: raw.slice(1) })
      newLine += 1
      continue
    }
    if (raw.startsWith('-')) continue
    if (raw.startsWith(' ')) newLine += 1
  }
  return out
}

/**
 * Scan the candidate's own commits for credential shapes. `readDiff` must return the diff of a
 * commit against its PARENT, or `null` when it cannot be read.
 */
export async function scanCandidateOwnDiff(input: {
  commits: ReadonlyArray<string>
  readDiff: (commit: string) => Promise<string | null>
}): Promise<CandidateSecretScan> {
  const findings: CandidateSecretFinding[] = []
  const unreadable: string[] = []
  const seen = new Set<string>()
  for (const commit of input.commits) {
    const diff = await input.readDiff(commit)
    if (diff === null) {
      unreadable.push(commit)
      continue
    }
    for (const added of parseAddedLines(diff)) {
      const key = `${added.file}\u0000${added.line}\u0000${added.text}`
      if (seen.has(key)) continue
      seen.add(key)
      for (const shape of secretShapesInLine(added.text)) {
        findings.push({ rule: shape.name, file: added.file, line: added.line })
      }
    }
  }
  return { findings, unreadable }
}
