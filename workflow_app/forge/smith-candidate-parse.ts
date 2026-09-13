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

export function parseSmithCandidate(notes: string | null | undefined): SmithCandidate | null {
  if (!notes) return null
  const lines = notes.split(/\r?\n/).map((s) => s.trim()).filter((s) => s.startsWith(SMITH_CANDIDATE_PREFIX))
  if (lines.length !== 1) return null
  try {
    const raw = JSON.parse(lines[0].slice(SMITH_CANDIDATE_PREFIX.length).trim()) as Record<string, unknown>
    if (raw.version !== 1) return null
    if (typeof raw.assignmentId !== 'string' || !raw.assignmentId.trim()) return null
    if (typeof raw.candidateSha !== 'string' || !SHA.test(raw.candidateSha)) return null
    if (typeof raw.mergeBase !== 'string' || !SHA.test(raw.mergeBase)) return null
    if (!Array.isArray(raw.changedPaths) || !raw.changedPaths.every((p) => typeof p === 'string' && p.trim())) {
      return null
    }
    return {
      version: 1,
      assignmentId: raw.assignmentId.trim(),
      candidateSha: raw.candidateSha.toLowerCase(),
      mergeBase: raw.mergeBase.toLowerCase(),
      changedPaths: raw.changedPaths.map((p) => String(p).trim()),
    }
  } catch {
    return null
  }
}
