// ---------------------------------------------------------------------------
// ENG-FORGE-LANE-SECRET-GATE-01 — the publish gate refuses a candidate that carries a credential.
//
// The fence drives BOTH directions against a real temporary git repository:
//   - a planted credential in the candidate's own commit is refused, naming the rule and file;
//   - a clean candidate publishes;
//   - a credential that exists only in history/base is not attributed to the candidate;
//   - the scan itself reports a finding for a planted line and none for a clean line.
//
// The planted value is assembled at runtime so this fence file never contains a credential-shaped
// literal of its own.
// ---------------------------------------------------------------------------

import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import test from 'node:test'

import {
  parseAddedLines,
  scanCandidateOwnDiff,
} from '../../lib/worker-workspace/candidate-secret-scan'
import { publishAcceptedCandidate } from '../../lib/worker-workspace/publish'

const run = promisify(execFile)

/** A value the catalog calls an `openai-style key`, built at runtime (no literal in this file). */
const PLANTED_CREDENTIAL = `sk-${'a'.repeat(20)}`

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await run(
    'git',
    ['-c', 'user.name=Fence', '-c', 'user.email=fence@example.test', ...args],
    { cwd },
  )
  return stdout.trim()
}

async function makeRepo(): Promise<{ root: string; cleanup: () => Promise<void> }> {
  const base = await mkdtemp(join(tmpdir(), 'lane-secret-gate-'))
  const root = join(base, 'work')
  await mkdir(root)
  await git(base, ['init', '--bare', join(base, 'origin.git')])
  await git(root, ['init'])
  await git(root, ['checkout', '-b', 'main'])
  await git(root, ['remote', 'add', 'origin', join(base, 'origin.git')])
  return {
    root,
    cleanup: async () => {
      await rm(base, { recursive: true, force: true })
    },
  }
}

async function commitFile(
  root: string,
  name: string,
  content: string,
  message: string,
): Promise<string> {
  await writeFile(join(root, name), content, 'utf8')
  await git(root, ['add', name])
  await git(root, ['commit', '-m', message])
  return git(root, ['rev-parse', 'HEAD'])
}

test('refuses a candidate whose diff adds a credential and names the rule and file', async () => {
  const { root, cleanup } = await makeRepo()
  try {
    const base = await commitFile(root, 'README.md', 'base\n', 'base')
    await git(root, ['push', 'origin', 'main'])
    const candidate = await commitFile(
      root,
      'config.ts',
      `export const token = "${PLANTED_CREDENTIAL}"\n`,
      'candidate',
    )

    const outcome = await publishAcceptedCandidate({ repoRoot: root, candidateCommit: candidate })

    assert.equal(outcome.outcome, 'candidate-secret')
    if (outcome.outcome !== 'candidate-secret') return
    assert.equal(outcome.findings[0]?.rule, 'openai-style key')
    assert.equal(outcome.findings[0]?.file, 'config.ts')
    assert.match(outcome.reason, /openai-style key/)
    assert.match(outcome.reason, /config\.ts/)

    // Nothing was pushed: origin/main is still the base commit.
    const remoteHead = (await git(root, ['ls-remote', 'origin', 'refs/heads/main'])).split(/\s+/)[0]
    assert.equal(remoteHead, base)
  } finally {
    await cleanup()
  }
})

test('passes a candidate with no credential', async () => {
  const { root, cleanup } = await makeRepo()
  try {
    await commitFile(root, 'README.md', 'base\n', 'base')
    await git(root, ['push', 'origin', 'main'])
    const candidate = await commitFile(root, 'feature.ts', 'export const value = 42\n', 'candidate')

    const outcome = await publishAcceptedCandidate({ repoRoot: root, candidateCommit: candidate })

    assert.equal(outcome.outcome, 'published')
  } finally {
    await cleanup()
  }
})

test('reads only the candidate own changes so a historical exposure does not block', async () => {
  const { root, cleanup } = await makeRepo()
  try {
    // The BASE commit already carries a credential-shaped value. It is in history, not in this
    // lane's change, so it must not block the candidate.
    await commitFile(root, 'legacy.ts', `export const legacy = "${PLANTED_CREDENTIAL}"\n`, 'base')
    await git(root, ['push', 'origin', 'main'])
    const candidate = await commitFile(root, 'clean.ts', 'export const ok = true\n', 'candidate')

    const outcome = await publishAcceptedCandidate({ repoRoot: root, candidateCommit: candidate })

    assert.equal(outcome.outcome, 'published')
  } finally {
    await cleanup()
  }
})

test('proves both directions with a planted key', async () => {
  const dirty = await scanCandidateOwnDiff({
    commits: ['candidate'],
    readDiff: async () =>
      [
        'diff --git a/config.ts b/config.ts',
        '--- a/config.ts',
        '+++ b/config.ts',
        '@@ -0,0 +1 @@',
        `+export const token = "${PLANTED_CREDENTIAL}"`,
        '',
      ].join('\n'),
  })
  assert.equal(dirty.findings.length, 1)
  assert.equal(dirty.findings[0]?.file, 'config.ts')

  const clean = await scanCandidateOwnDiff({
    commits: ['candidate'],
    readDiff: async () =>
      [
        'diff --git a/config.ts b/config.ts',
        '--- a/config.ts',
        '+++ b/config.ts',
        '@@ -0,0 +1 @@',
        '+export const token = process.env.TOKEN',
        '',
      ].join('\n'),
  })
  assert.equal(clean.findings.length, 0)

  // A deleted credential is not an added credential.
  const deleted = parseAddedLines(
    ['diff --git a/config.ts b/config.ts', '--- a/config.ts', '+++ b/config.ts', '@@ -1 +0,0 @@', `-${PLANTED_CREDENTIAL}`].join('\n'),
  )
  assert.equal(deleted.length, 0)
})
