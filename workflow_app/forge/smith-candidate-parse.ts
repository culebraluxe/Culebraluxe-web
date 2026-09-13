import { lastJsonObjectMatching } from './agents/shared/json-slice'

export const SMITH_CANDIDATE_PREFIX = 'SMITH_CANDIDATE:'

export const SMITH_CANDIDATE_MISSING =
  'SMITH: no candidate. End with exactly one un-fenced JSON line SMITH_CANDIDATE: {"version":1,"assignmentId":"...","candidateSha":"<sha>","mergeBase":"<sha>","changedPaths":["file.ts"]}'

export type SmithCandidate = {
  version: 1
  assignmentId: string
  candidateSha: string
  mergeBase: string
  changedPaths: string[]
}

const SHA = /^[0-9a-f]{7,40}$/i

/** Validate a parsed row into a candidate, or null. */
function candidateFromRow(raw: unknown): SmithCandidate | null {
  if (!raw || typeof raw !== 'object') return null
  const row = raw as Record<string, unknown>
  if (row.version !== 1) return null
  if (typeof row.assignmentId !== 'string' || !row.assignmentId.trim()) return null
  if (typeof row.candidateSha !== 'string' || !SHA.test(row.candidateSha)) return null
  if (typeof row.mergeBase !== 'string' || !SHA.test(row.mergeBase)) return null
  if (!Array.isArray(row.changedPaths) || !row.changedPaths.every((p) => typeof p === 'string' && p.trim())) {
    return null
  }
  return {
    version: 1,
    assignmentId: row.assignmentId.trim(),
    candidateSha: row.candidateSha.toLowerCase(),
    mergeBase: row.mergeBase.toLowerCase(),
    changedPaths: row.changedPaths.map((p) => String(p).trim()),
  }
}

export function parseSmithCandidate(notes: string | null | undefined): SmithCandidate | null {
  if (!notes) return null
  const lines = notes
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.startsWith(SMITH_CANDIDATE_PREFIX))

  if (lines.length > 1) return null // duplicated marker lines are ambiguous
  if (lines.length === 1) {
    try {
      const fromLine = candidateFromRow(JSON.parse(lines[0].slice(SMITH_CANDIDATE_PREFIX.length).trim()))
      if (fromLine) return fromLine
    } catch {
      /* fall through to the shape scan */
    }
  }

  // NO marker. A missing LABEL is a formatting slip, not a missing candidate — the
  // same tolerance every other contract marker got (see shared/json-slice.ts). The
  // shape check keeps this safe: only a version-1 object with an assignmentId and a
  // candidate SHA can be picked up, so nothing else in a Smith reply can be mistaken
  // for its candidate. Git still overrules the claim downstream.
  const row = lastJsonObjectMatching(
    notes,
    (r) => r.version === 1 && typeof r.candidateSha === 'string' && typeof r.assignmentId === 'string',
  )
  return row ? candidateFromRow(row) : null
}

