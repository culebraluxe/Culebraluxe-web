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
import { mkdtemp, rm, symlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'

import { commitOwnDiffReader, scanCandidateOwnDiff, type CandidateSecretFinding } from './candidate-secret-scan'
import { listCommitsToPublish } from './publish-range'

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
  /**
   * Re-verify an INTEGRATED candidate before it is published.
   *
   * Supplied by the engine, which knows the story's frozen proofs. It runs in the integration
   * worktree, against the integrated commit, and its refusal is FINAL: an integration that
   * cannot be verified is not published. Without this callback a moved remote still fails
   * closed exactly as before, so no existing caller changes behaviour.
   */
  verifyIntegrated?: (input: {
    cwd: string
    integratedCommit: string
  }) => Promise<{ ok: boolean; detail?: string | null }>
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
  /**
   * The candidate's own changes add a credential-shaped line. Refused BEFORE any remote is read,
   * so the credential never reaches `origin/main` and never becomes history.
   */
  | {
      outcome: 'candidate-secret'
      candidateCommit: string
      /**
       * Set when the credential-shaped value was found in the INTEGRATION commit rather than in the candidate
       * range (work package C): the integration authors a new commit, and it is scanned before it is pushed.
       */
      integratedCommit?: string
      findings: CandidateSecretFinding[]
      reason: string
    }
  | {
      outcome: 'publish-conflict'
      candidateCommit: string | null
      remoteMainHash: string | null
      reason: string
    }
  /**
   * The remote had advanced, the candidate was integrated onto the CURRENT remote head, the
   * integrated tree was re-verified, and THAT commit was published. `candidateCommit` is the
   * Smith's original commit (preserved on its own branch); `integratedCommit` is what main
   * received.
   */
  | {
      outcome: 'integrated-and-published'
      candidateCommit: string
      integratedCommit: string
      publishedMainHash: string
      verifyDetail: string | null
    }
  /** Integration was attempted, produced a commit, and the verifier refused it: NOT published. */
  | {
      outcome: 'integration-unverified'
      candidateCommit: string
      integratedCommit: string
      reason: string
    }
  /** Integration itself conflicted (a REAL conflict, not a moved remote). Nothing published. */
  | {
      outcome: 'integration-conflict'
      candidateCommit: string
      remoteMainHash: string
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
 * INTEGRATE A CANDIDATE ONTO THE CURRENT REMOTE HEAD, in a scratch worktree.
 *
 * The candidate's branch is never rewritten and `main` is never touched here: a detached
 * worktree is created at the candidate, the remote head is merged into it, and the resulting
 * commit is handed back for verification. The caller publishes only if verification passes,
 * so a merge that compiles is not the same thing as a merge that is PROVEN.
 *
 * A real conflict (the same lines changed on both sides) is reported as such; a moved remote
 * alone is what this exists to stop being fatal.
 */
async function integrateCandidateWithRemote(input: {
  repoRoot: string
  candidate: string
  remoteMain: string
  remoteName: string
  remoteBranch: string
}): Promise<
  | { outcome: 'integrated'; commit: string; dir: string; cleanup: () => Promise<void> }
  | { outcome: 'conflict'; reason: string }
> {
  const dir = await mkdtemp(join(tmpdir(), `forge-integrate-${input.candidate.slice(0, 8)}-`))
  const cleanup = async (): Promise<void> => {
    await runGit(input.repoRoot, ['worktree', 'remove', '--force', dir])
    await rm(dir, { recursive: true, force: true })
  }

  const add = await runGit(input.repoRoot, ['worktree', 'add', '--detach', dir, input.candidate])
  if (!add.ok) {
    await rm(dir, { recursive: true, force: true })
    return { outcome: 'conflict', reason: `could not create the integration worktree: ${add.stderr}` }
  }

  // THE INTEGRATION TREE NEEDS THE DEPENDENCIES, OR EVERY PROOF FAILS FOR THE WRONG REASON.
  //
  // A fresh `git worktree` carries source and nothing else: no node_modules, no .env. The frozen
  // proofs are commands like `node --import tsx --test <file>`, so in a bare worktree they fail to
  // even start — `tsx` is not resolvable — and the publisher then reports `integration-unverified`
  // for a candidate that integrates cleanly and passes its proofs in a real checkout. Measured by
  // hand on 2026-09-14 (candidate 70738a12, main one commit ahead): merge clean, proof 13/13 pass in
  // the repo, and the engine refused it. Linking the repo's own node_modules removes the false
  // negative without weakening anything: the proofs still run against the INTEGRATED SOURCE, which is
  // what this step is for.
  await symlink(join(input.repoRoot, 'node_modules'), join(dir, 'node_modules')).catch(() => {
    /* best effort: a proof that needs a missing dependency will still fail loudly, and named */
  })

  // Merge the CURRENT remote head, so the integrated tree is what main would become.
  const merge = await runGit(dir, ['merge', '--no-edit', input.remoteMain])
  if (!merge.ok) {
    await runGit(dir, ['merge', '--abort'])
    await cleanup()
    return {
      outcome: 'conflict',
      reason: `merging ${input.remoteName}/${input.remoteBranch} (${input.remoteMain.slice(0, 12)}) into ${input.candidate.slice(0, 12)} conflicted (${merge.stderr})`,
    }
  }

  const head = await runGit(dir, ['rev-parse', 'HEAD'])
  if (!head.ok || !head.stdout) {
    await cleanup()
    return { outcome: 'conflict', reason: 'the integration worktree has no readable HEAD' }
  }

  // A clean merge that changes nothing is not an integration: publishing the candidate would
  // still not fast-forward, and pretending otherwise would hide the real state.
  if (head.stdout === input.candidate) {
    await cleanup()
    return {
      outcome: 'conflict',
      reason:
        `merging ${input.remoteMain.slice(0, 12)} into ${input.candidate.slice(0, 12)} produced no new ` +
        'commit, so the remote head is still not an ancestor of anything publishable',
    }
  }

  return { outcome: 'integrated', commit: head.stdout, dir, cleanup }
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

  // EVERY COMMIT THE PUSH WOULD SEND, SCANNED BEFORE THE REMOTE IS READ.
  //
  // This used to pass `commits: [candidate]` — the tip only — so a credential introduced in an EARLIER
  // unpublished commit rode along behind a clean final commit (FORGE-PUBLISH-SCAN-COVERAGE-01, reproduced by the
  // Astra review: tip-only found 0 findings where both unpublished commits found 1). The range is now measured
  // from THE DESTINATION'S OWN ANSWER (work package C): local main is not evidence about the remote, and a
  // remote-tracking ref is a cache that can be stale, so either one can omit commits this push will send.
  //
  // An unreadable diff still fails closed: an unscanned candidate is not a clean candidate.
  const publishRange = await listCommitsToPublish({
    repoRoot,
    candidate,
    remoteName,
    remoteBranch,
  })
  // THE RANGE COULD NOT BE ESTABLISHED: refuse BEFORE reading the remote, and never fall back to a tip-only scan.
  // "We could not read the destination" is not "there is nothing else to scan".
  if (publishRange.unreadable) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: null,
      reason:
        `could not establish the commits to publish for candidate ${candidate.slice(0, 12)} ` +
        `(${publishRange.source}): ${publishRange.refusal ?? 'the range could not be read'}. Refusing to publish ` +
        'a candidate whose history could not be read.',
    }
  }
  const secretScan = await scanCandidateOwnDiff({
    commits: publishRange.commits,
    // THE ROOT COMMIT IS READABLE (work package C): a parentless commit's own diff is its whole tree, taken
    // against the empty tree. Without this, a first publish onto a new branch reported `unreadable` and refused —
    // the right answer for a genuinely unreadable commit, and the wrong one for a root commit.
    readDiff: commitOwnDiffReader(repoRoot),
  })
  if (secretScan.unreadable.length > 0) {
    return {
      outcome: 'publish-conflict',
      candidateCommit: candidate,
      remoteMainHash: null,
      reason:
        `could not read the candidate's own diff for ${secretScan.unreadable
          .map((commit) => commit.slice(0, 12))
          .join(', ')}; refusing to publish a candidate whose changes could not be scanned`,
    }
  }
  if (secretScan.findings.length > 0) {
    const named = secretScan.findings
      .map(
        (finding) =>
          `${finding.rule} in ${finding.file}:${finding.line} (introduced by ${finding.commit.slice(0, 12)})`,
      )
      .join(', ')
    return {
      outcome: 'candidate-secret',
      candidateCommit: candidate,
      findings: secretScan.findings,
      reason: `candidate ${candidate.slice(0, 12)} adds a credential-shaped value (${named}); publication refused after scanning ${publishRange.commits.length} commit(s) of the range based on ${publishRange.source}`,
    }
  }

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
    const movedReason =
      ancestor.code === 1
        ? `origin/${remoteBranch} (${remoteMain}) is not an ancestor of candidate ${candidate} — remote main has advanced from the candidate's recorded base`
        : `fast-forward check failed: ${ancestor.stderr || 'git merge-base error'}`

    // INTEGRATE BEFORE REFUSING (2026-09-13).
    //
    // This used to be the end of the road: any advance of origin/main made the candidate
    // unpublishable, and the engine answered repair → re-verify → publish → refuse until the
    // turn cap. With main moving several times an hour (parallel work, the engine's own
    // publishes), that was not an edge case — it was every story that started before the last
    // commit landed, which is why a night of runs could not finish.
    //
    // A retry should be able to finish, so when a verifier is supplied the candidate is
    // integrated onto the CURRENT remote head in a SCRATCH worktree, the frozen proofs are
    // re-run against the integrated tree, and THAT commit is published. Invariants preserved:
    // main is only ever fast-forwarded, nothing is force-pushed, the Smith's commit stays on
    // its own branch, a real conflict fails closed, and an unverifiable integration is not
    // published at all.
    if (!input.verifyIntegrated) {
      return {
        outcome: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: remoteMain,
        reason: `${movedReason}. Candidate commit preserved; refusing to force-push or rewrite history.`,
      }
    }

    const integrated = await integrateCandidateWithRemote({
      repoRoot,
      candidate,
      remoteMain,
      remoteName,
      remoteBranch,
    })
    if (integrated.outcome === 'conflict') {
      return {
        outcome: 'integration-conflict',
        candidateCommit: candidate,
        remoteMainHash: remoteMain,
        reason: `${movedReason}; INTEGRATION CONFLICT: ${integrated.reason}. Candidate preserved, nothing published.`,
      }
    }

    const verification = await input.verifyIntegrated({
      cwd: integrated.dir,
      integratedCommit: integrated.commit,
    })
    if (!verification.ok) {
      await integrated.cleanup()
      return {
        outcome: 'integration-unverified',
        candidateCommit: candidate,
        integratedCommit: integrated.commit,
        reason:
          `candidate ${candidate} was integrated onto ${remoteMain} as ${integrated.commit}, ` +
          `but the integrated tree did not verify: ${verification.detail ?? 'verifier refused'}. ` +
          'Nothing was published — the proofs decide, not the merge.',
      }
    }

    // THE INTEGRATED COMMIT IS A NEW COMMIT, SO IT GETS ITS OWN SCAN (work package C, item 8). Integration
    // authors a commit the range scan above never saw — a merge whose resolution can introduce anything — and
    // pushing it unscanned would reintroduce exactly the hole this story closes, one layer up. The scan reads
    // the integrated commit's own diff against its first parent, so it covers the candidate's changes AND the
    // resolution, and a finding refuses the publication with the worktree cleaned up.
    const integratedScan = await scanCandidateOwnDiff({
      commits: [integrated.commit],
      readDiff: commitOwnDiffReader(repoRoot),
    })
    if (integratedScan.unreadable.length > 0 || integratedScan.findings.length > 0) {
      const named = integratedScan.findings
        .map((finding) => `${finding.rule} in ${finding.file}:${finding.line}`)
        .join(', ')
      const reason = integratedScan.unreadable.length
        ? `the integrated commit ${integrated.commit.slice(0, 12)} could not be scanned; nothing was published`
        : `the integrated commit ${integrated.commit.slice(0, 12)} adds a credential-shaped value (${named}); publication refused`
      await integrated.cleanup()
      return {
        outcome: 'candidate-secret',
        candidateCommit: candidate,
        integratedCommit: integrated.commit,
        findings: integratedScan.findings,
        reason,
      }
    }

    // Publish the INTEGRATED commit. It descends from the current remote head, so this is a
    // genuine fast-forward, and the proof that it was verified rides the outcome.
    const pushIntegrated = await runGit(
      repoRoot,
      ['push', remoteName, `${integrated.commit}:refs/heads/${remoteBranch}`],
      { env: { ...process.env, GIT_TERMINAL_PROMPT: '0' } },
    )
    await runGit(repoRoot, [
      'update-ref',
      `refs/remotes/${remoteName}/${remoteBranch}`,
      integrated.commit,
    ])
    const detail = verification.detail ?? null
    await integrated.cleanup()
    if (!pushIntegrated.ok) {
      return {
        outcome: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: remoteMain,
        reason:
          `integrated commit ${integrated.commit} was rejected by ${remoteName}/${remoteBranch} ` +
          `(${pushIntegrated.stderr || 'git push failed'}). Candidate preserved; no force-push is ever used.`,
      }
    }
    return {
      outcome: 'integrated-and-published',
      candidateCommit: candidate,
      integratedCommit: integrated.commit,
      publishedMainHash: integrated.commit,
      verifyDetail: detail,
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
