/**
 * WHICH COMMITS A PUSH WOULD ACTUALLY SEND (FORGE-PUBLISH-SCAN-COVERAGE-01).
 *
 * The credential scan used to receive `commits: [candidate]` — the tip only — so a credential introduced
 * in an EARLIER unpublished commit rode along behind a clean final commit. Astra reproduced it: a tip-only
 * scan found 0 findings where scanning both unpublished commits found 1. The scan was never wrong about
 * what it read; it was never asked about the other commits.
 *
 * The set is derived from a BASE, in this order of preference, and the source is returned so a caller can
 * say which it used rather than implying it knew more than it did:
 *
 *   remote-tracking  `refs/remotes/<remote>/<branch>` — history already on the remote, as far as this
 *                    clone knows. Preferred, because it is exactly the boundary the push will use.
 *   local-base       `refs/heads/<fallback>` (default `main`) — the story base, always present in the
 *                    primary checkout. Used when no tracking ref exists yet (a fresh clone that has
 *                    pushed nothing), which is the common case for the first publish of a new checkout.
 *   candidate-only   no base resolvable at all. The candidate's own diff is then all that can be
 *                    attributed, and saying so is better than silently scanning the entire history.
 *
 * A commit whose diff cannot be read is NOT skipped here — the scanner already fails closed on an
 * unreadable commit (an unscanned candidate is not a clean candidate) and that property is preserved.
 */
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<{ ok: boolean; stdout: string }> {
  try {
    const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
    return { ok: true, stdout: stdout.trim() }
  } catch {
    return { ok: false, stdout: '' }
  }
}

export type PublishRangeSource = 'remote-tracking' | 'local-base' | 'candidate-only'

export type PublishRange = {
  /** Commits the push would send, newest first, and always including the candidate itself. */
  commits: string[]
  /** The commit the range was measured from, or null when no base could be resolved. */
  base: string | null
  source: PublishRangeSource
  /**
   * True when a base WAS resolved but the range could not be listed. That is not the same as
   * `candidate-only`: history exists and could not be read, so the caller must refuse rather than scan
   * the tip and report it as complete coverage.
   */
  unreadable: boolean
}

export async function listCommitsToPublish(input: {
  repoRoot: string
  candidate: string
  remoteName: string
  remoteBranch: string
  /** Local branch used when no remote-tracking ref exists. Default `main`. */
  fallbackBase?: string
}): Promise<PublishRange> {
  const tracking = `refs/remotes/${input.remoteName}/${input.remoteBranch}`
  const candidates: Array<{ ref: string; source: PublishRangeSource }> = [
    { ref: tracking, source: 'remote-tracking' },
    { ref: `refs/heads/${input.fallbackBase ?? 'main'}`, source: 'local-base' },
  ]

  for (const { ref, source } of candidates) {
    const resolved = await git(input.repoRoot, ['rev-parse', '--verify', '--quiet', ref])
    if (!resolved.ok || !resolved.stdout) continue
    const base = resolved.stdout
    // A base exists, so the range is knowable. If `rev-list` cannot list it, the history is UNREADABLE:
    // returning just the candidate here would silently reintroduce the tip-only hole this module exists
    // to close. The caller must refuse.
    const range = await git(input.repoRoot, ['rev-list', `${base}..${input.candidate}`])
    if (!range.ok) {
      return { commits: [input.candidate], base, source, unreadable: true }
    }
    const commits = range.stdout.split('\n').map((l) => l.trim()).filter(Boolean)
    return {
      commits: [input.candidate, ...commits.filter((c) => c !== input.candidate)],
      base,
      source,
      unreadable: false,
    }
  }

  // No base resolvable at all. The candidate's own diff is all that can be attributed; this is an
  // explicit state, not a failed read, so it is not `unreadable`.
  return { commits: [input.candidate], base: null, source: 'candidate-only', unreadable: false }
}
