/**
 * WHICH COMMITS A PUSH WOULD ACTUALLY SEND (FORGE-PUBLISH-SCAN-COVERAGE-01, work package C).
 *
 * The credential scan used to receive `commits: [candidate]` — the tip only — so a credential introduced in an
 * EARLIER unpublished commit rode along behind a clean final commit. Astra reproduced it: a tip-only scan found
 * 0 findings where scanning both unpublished commits found 1. The scan was never wrong about what it read; it
 * was never asked about the other commits.
 *
 * THE BOUNDARY COMES FROM THE DESTINATION, NOT FROM A LOCAL REF. The first version of this module preferred
 * `refs/remotes/<remote>/<branch>` and fell back to local `main` — and Astra reproduced the second half of the
 * defect: with local main pointing at the candidate and NO tracking ref, the range came out empty and only the
 * tip was scanned, while two real unpublished commits existed. Local main is not evidence about the remote, and
 * a tracking ref is a CACHE of what the remote had when this clone last looked: a stale cache omits commits
 * that are actually being published. So the boundary is asked for at the destination:
 *
 *   remote-ref   `git ls-remote <remote> refs/heads/<branch>` answered and the branch exists → that sha is the
 *                boundary the push itself will use. Authoritative, so staleness cannot apply.
 *   new-branch   the remote answered and the branch DOES NOT EXIST. That answer is authoritative too, and it
 *                means every commit reachable from the candidate is introduced by this push — including the
 *                root commit — so all of that history is scanned. A superset of what the push sends, which is
 *                the safe direction to be wrong in.
 *   unreadable   the remote could not be read (network, auth, unknown remote), or the boundary it named is not
 *                present in this clone so the range cannot be established. NOT a silent candidate-only:
 *                `unreadable` is true, the reason is set, and the caller must refuse.
 *
 * `candidate-only` is gone as a success state. There is no situation in which we know coverage is incomplete
 * and are entitled to report a clean scan of one commit.
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

export type PublishRangeSource = 'remote-ref' | 'new-branch' | 'unreadable'

export type PublishRange = {
  /** Commits the push would send, newest first, and always including the candidate itself. */
  commits: string[]
  /** The commit the range was measured from, or null for a new branch (nothing to measure from). */
  base: string | null
  source: PublishRangeSource
  /**
   * True when the range could not be established or listed. An unscanned candidate is not a clean candidate,
   * so the caller must REFUSE rather than scan the tip and call it complete coverage.
   */
  unreadable: boolean
  /**
   * True for a new destination branch, where the range is the candidate's whole history because every commit in
   * it is introduced by this push. Reported so a slow scan is explained rather than mysterious.
   */
  fullHistory: boolean
  /** Why the range could not be established, for the refusal the caller records. Null when it could. */
  refusal: string | null
}

export async function listCommitsToPublish(input: {
  repoRoot: string
  candidate: string
  remoteName: string
  remoteBranch: string
}): Promise<PublishRange> {
  const destination = `${input.remoteName}/${input.remoteBranch}`
  const unreadable = (refusal: string): PublishRange => ({
    commits: [input.candidate],
    base: null,
    source: 'unreadable',
    unreadable: true,
    fullHistory: false,
    refusal,
  })

  // THE DESTINATION IS ASKED DIRECTLY. `ls-remote` reports what the remote has NOW; a remote-tracking ref reports
  // what it had when this clone last looked, which is exactly how commits get published unscanned.
  const remote = await git(input.repoRoot, ['ls-remote', input.remoteName, `refs/heads/${input.remoteBranch}`])
  if (!remote.ok) {
    return unreadable(
      `the destination ${destination} could not be read (git ls-remote failed), so the range this push would ` +
        'send is unknown; a candidate scanned alone is not a scanned push',
    )
  }

  const remoteSha = remote.stdout.split(/\s+/)[0]?.trim() ?? ''
  if (!/^[0-9a-f]{40}$/.test(remoteSha)) {
    // AN AUTHORITATIVE "DOES NOT EXIST" IS NOT AN UNREADABLE ANSWER. Every commit reachable from the candidate is
    // introduced by this push, so the whole history is the range — the root commit included.
    const all = await git(input.repoRoot, ['rev-list', input.candidate])
    if (!all.ok) {
      return unreadable(`the history reachable from candidate ${input.candidate} could not be listed`)
    }
    const commits = all.stdout.split('\n').map((line) => line.trim()).filter(Boolean)
    return {
      commits: commits.includes(input.candidate) ? commits : [input.candidate, ...commits],
      base: null,
      source: 'new-branch',
      unreadable: false,
      fullHistory: true,
      refusal: null,
    }
  }

  // THE BOUNDARY MUST BE PRESENT LOCALLY, or the range cannot be established. This clone may simply not have
  // fetched what the remote has; "we do not have the commit the range starts from" refuses rather than guessing.
  const present = await git(input.repoRoot, ['cat-file', '-e', `${remoteSha}^{commit}`])
  if (!present.ok) {
    return unreadable(
      `the destination ${destination} is at ${remoteSha} but this clone does not contain that commit, so the ` +
        'range this push would send cannot be established',
    )
  }

  const range = await git(input.repoRoot, ['rev-list', `${remoteSha}..${input.candidate}`])
  if (!range.ok) {
    return unreadable(
      `the range ${remoteSha}..${input.candidate} could not be listed, so the commits this push would send are ` +
        'unknown',
    )
  }
  const commits = range.stdout.split('\n').map((line) => line.trim()).filter(Boolean)
  return {
    commits: [input.candidate, ...commits.filter((commit) => commit !== input.candidate)],
    base: remoteSha,
    source: 'remote-ref',
    unreadable: false,
    fullHistory: false,
    refusal: null,
  }
}
