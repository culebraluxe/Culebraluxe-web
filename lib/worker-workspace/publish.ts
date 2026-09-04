// ---------------------------------------------------------------------------
// ENG-FORGE-V4-10B — outer-Forge publication of an accepted candidate to the
// deployment source (`origin/main`).
//
// This module runs in the OUTER Forge process (the worker/scheduler host that
// owns the git checkout and `origin`), NEVER inside the model sandbox. Git is
// the repository's own VCS — the same boring local/remote plumbing the
// provisioner and commit helper use.
//
// Publication is a post-Assay acceptance action, not part of Smith execution.
// It publishes one accepted Smith candidate commit directly to
// `origin/main` ONLY when that is a safe NON-FORCE fast-forward:
//
//   a real candidate commit is present locally
//     && remote main is an ancestor of the candidate (exactly the candidate's
//        recorded base for a normal harness-created candidate, because the
//        candidate branch is created from that base and never rewritten)
//     && `git push` (never --force) succeeds
//
// Any missing/unresolvable candidate, missing remote, remote divergence, or
// rejected push fails closed into a factual `publish-conflict` outcome. The
// candidate commit is NEVER discarded and `main` is NEVER force-pushed or
// rewritten. No merge/rebase/PR ceremony exists here.
// ---------------------------------------------------------------------------

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

export const DEFAULT_PUBLISH_REMOTE = 'origin'
export const DEFAULT_PUBLISH_BRANCH = 'main'

export type PublishAcceptedCandidateInput = {
  /** Primary checkout root that owns the `origin` remote (outer Forge host). */
  repoRoot: string
  /** Smith candidate commit hash recorded in run evidence. Null/empty = no candidate. */
  candidateCommit: string | null | undefined
  /** Remote owning the deployment branch (default `origin`). */
  remoteName?: string
  /** Deployment branch to publish onto (default `main`). Never force-pushed. */
  remoteBranch?: string
}

export type PublishAcceptedCandidateOutcome =
  | {
      outcome: 'published'
      candidateCommit: string
      /** The `origin/main` head after publication (== the candidate commit). */
      publishedMainHash: string
    }
  | {
      outcome: 'no-candidate'
      reason: string
    }
  | {
      outcome: 'publish-conflict'
      candidateCommit: string | null
      remoteMainHash: string | null
      reason: string
    }

type GitResult = {
  ok: boolean
  code: number | null
  stdout: string
  stderr: string
}

async function runGit(
  cwd: string,
  args: string[],
  opts?: { env?: NodeJS.ProcessEnv },
): Promise<GitResult> {
  try {
    const { stdout, stderr } = await execFileAsync('git', args, {
      cwd,
      encoding: 'utf8',
      ...opts,
    })
    return { ok: true, code: 0, stdout: stdout.trim(), stderr: stderr.trim() }
  } catch (err) {
    const e = err as {
      code?: number | string
      stdout?: string
      stderr?: string
      message: string
    }
    return {
      ok: false,
      code: typeof e.code === 'number' ? e.code : null,
      stdout: ((e.stdout as string | undefined) ?? '').trim(),
      stderr: ((e.stderr as string | undefined) ?? '').trim() || e.message,
    }
  }
}

/** First whitespace-separated token of a `git ls-remote` line (the ref hash). */
function refHash(line: string): string {
  return line.trim().split(/\s+/)[0] ?? ''
}

/**
 * Publish an accepted candidate commit to `origin/main` when (and only when)
 * that is a safe fast-forward. Returns a typed outcome; never throws for
 * publish decisions and never uses `--force`.
 */
export async function publishAcceptedCandidate(
  input: PublishAcceptedCandidateInput,
): Promise<PublishAcceptedCandidateOutcome> {
  const repoRoot = input.repoRoot
  const remoteName = input.remoteName?.trim() || DEFAULT_PUBLISH_REMOTE
  const remoteBranch = input.remoteBranch?.trim() || DEFAULT_PUBLISH_BRANCH
  const rawCandidate = (input.candidateCommit ?? '').trim()

  const repoCheck = await runGit(repoRoot, ['rev-parse', '--git-dir'])
  if (!repoCheck.ok) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: rawCandidate || null,
      remoteMainHash: null,
      reason: `publish requires a git repository at ${repoRoot} (${repoCheck.stderr || 'not a git repository'})`,
    }
  }

  if (!rawCandidate) {
    return {
      outcome: 'no-candidate',
      reason:
        'no candidate commit hash was supplied; publication requires a real Smith candidate commit',
    }
  }

  // The candidate must actually exist locally before anything is offered to
  // the remote. An unresolvable hash is an anomaly, not an empty candidate.
  const resolved = await runGit(repoRoot, [
    'rev-parse',
    '--verify',
    '--quiet',
    `${rawCandidate}^{commit}`,
  ])
  if (!resolved.ok || !resolved.stdout) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: rawCandidate,
      remoteMainHash: null,
      reason: `candidate commit ${rawCandidate.slice(0, 12)} is not present in the local repository; nothing can be pushed and the candidate is preserved on its branch for repair/retry`,
    }
  }
  const candidate = resolved.stdout

  // Current remote head — the remote is the authority for main, never a stale
  // local guess.
  const remoteLs = await runGit(repoRoot, [
    'ls-remote',
    remoteName,
    `refs/heads/${remoteBranch}`,
  ])
  if (!remoteLs.ok) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: null,
      reason: `cannot read remote ${remoteName}: ${remoteLs.stderr || 'ls-remote failed'}`,
    }
  }
  const remoteLine = remoteLs.stdout.split('\n').map((line) => line.trim()).find(Boolean)
  if (!remoteLine) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: null,
      reason: `remote ${remoteName} has no ${remoteBranch} branch (refs/heads/${remoteBranch}); refusing to create a deployment branch implicitly`,
    }
  }
  const remoteMain = refHash(remoteLine)

  // Idempotent success: the accepted code is already the remote head (e.g. a
  // retry after a partial/confirmed push). Synchronize the integration
  // tracking ref best-effort so successors branch from accepted state.
  if (remoteMain === candidate) {
    await runGit(repoRoot, [
      'update-ref',
      `refs/remotes/${remoteName}/${remoteBranch}`,
      candidate,
    ])
    return {
      outcome: 'published',
      candidateCommit: candidate,
      publishedMainHash: remoteMain,
    }
  }

  // Safe fast-forward check: a plain push only advances main when the current
  // remote head is an ancestor of the candidate. For a normal harness-created
  // candidate that ancestor IS the candidate's recorded base/current remote
  // main; anything else means main advanced or diverged and direct
  // publication is impossible without force or rewrite — both forbidden.
  const ancestor = await runGit(repoRoot, [
    'merge-base',
    '--is-ancestor',
    remoteMain,
    candidate,
  ])
  if (!ancestor.ok) {
    const reason =
      ancestor.code === 1
        ? `origin/${remoteBranch} (${remoteMain}) is not an ancestor of candidate ${candidate} — remote main has advanced or diverged from the candidate's recorded base, so a direct push would NOT fast-forward. Candidate commit preserved; refusing to force-push or rewrite history. Integrate/repair before republishing.`
        : `fast-forward check failed: ${ancestor.stderr || 'git merge-base error'}`
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: remoteMain,
      reason,
    }
  }

  // Direct push, never --force. GIT_TERMINAL_PROMPT=0 keeps an unattended
  // publish from hanging on an interactive credential prompt; a missing
  // credential fails closed like any other rejected push.
  const push = await runGit(
    repoRoot,
    ['push', remoteName, `${candidate}:refs/heads/${remoteBranch}`],
    { env: { ...process.env, GIT_TERMINAL_PROMPT: '0' } },
  )
  if (!push.ok) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: remoteMain,
      reason: `direct push of candidate ${candidate} to ${remoteName}/${remoteBranch} was rejected (${push.stderr || 'git push failed'}). Candidate preserved; no force-push is ever used.`,
    }
  }

  // Verify the factual post-publish head rather than trusting push output.
  const verify = await runGit(repoRoot, [
    'ls-remote',
    remoteName,
    `refs/heads/${remoteBranch}`,
  ])
  const verifiedHead = verify.ok ? refHash(verify.stdout) : ''
  if (verifiedHead !== candidate) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: remoteMain,
      reason: `push reported success but ${remoteName}/${remoteBranch} does not point at candidate ${candidate} (head is ${verifiedHead || 'unreadable'}); treating the publication as unresolved`,
    }
  }

  // ENG-FORGE-V5-03R / Invariant 8: synchronize the remote-tracking ref with
  // the accepted integration state so successor worktrees branch from the
  // newly-published head instead of a stale local checkout.
  await runGit(repoRoot, [
    'update-ref',
    `refs/remotes/${remoteName}/${remoteBranch}`,
    candidate,
  ])

  return {
    outcome: 'published',
    candidateCommit: candidate,
    publishedMainHash: verifiedHead,
  }
}
