import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { revokeForbiddenCommit, writeBoundaryLines } from './write-policy'

// WHY THIS FILE EXISTS: `docs/agent/packets/FORGE-GATES-01.md` lists it in its Assay commands and it was
// never written, so that packet's allow-list named a file that did not exist and the harness lint recorded
// it as baselined debt (rule 10). The module is the repository boundary for "a role that may not commit",
// which is one of the seeded factory invariants (`write policy` / "a rule promoted by its implementer is
// not a rule"), so the boundary deserves the assertion rather than the debt line.

test('a builder is told it may commit locally and must not push', () => {
  const lines = writeBoundaryLines({ allowCommit: true, allowDevDbWrite: false })
  const prose = lines.join('\n')
  assert.match(prose, /Create a local git commit/)
  assert.match(prose, /Do NOT push/)
  assert.doesNotMatch(prose, /may NOT create a git commit/)
})

test('a reader is told it may not commit, may not write the database, and must leave edits unstaged', () => {
  const prose = writeBoundaryLines({ allowCommit: false, allowDevDbWrite: false }).join('\n')
  assert.match(prose, /may NOT create a git commit and may NOT write DEV or PROD schema\/data/)
  assert.match(prose, /leave them unstaged/)
  assert.match(prose, /Do NOT push/)
  assert.doesNotMatch(prose, /Create a local git commit/)
})

test('a builder commit is left alone — there is nothing to revoke', () => {
  const result = revokeForbiddenCommit({
    allowCommit: true,
    workspace: '/tmp/not-used',
    commitHash: 'deadbee',
  })
  assert.deepEqual(result, { commitHash: 'deadbee', violation: null })
})

test('no commit means no violation, whoever the role is', () => {
  const result = revokeForbiddenCommit({
    allowCommit: false,
    workspace: '/tmp/not-used',
    commitHash: null,
  })
  assert.deepEqual(result, { commitHash: null, violation: null })
})

test('a non-builder commit is dropped from the evidence even when the rewind cannot run', () => {
  const result = revokeForbiddenCommit({
    allowCommit: false,
    // A workspace that is not a git repository: the rewind's failure is swallowed ON PURPOSE, because
    // the violation still has to be recorded. This is the assertion that the swallow cannot hide it.
    workspace: join(tmpdir(), 'culebraluxe-not-a-repo'),
    commitHash: 'abc1234',
  })
  assert.equal(result.commitHash, null)
  assert.match(String(result.violation), /WRITE POLICY VIOLATION: non-builder created commit abc1234/)
  assert.match(String(result.violation), /hash dropped and worktree rewind requested/)
})

test('a non-builder commit is rewound out of the worktree when base is known', () => {
  const dir = mkdtempSync(join(tmpdir(), 'culebraluxe-write-policy-'))
  try {
    const git = (...args: string[]) =>
      execFileSync('git', args, { cwd: dir, encoding: 'utf8' }).trim()
    git('init', '-q')
    git('config', 'user.email', 'test@example.invalid')
    git('config', 'user.name', 'write-policy test')
    writeFileSync(join(dir, 'base.txt'), 'base\n')
    git('add', '-A')
    git('commit', '-q', '-m', 'base')
    const base = git('rev-parse', 'HEAD')
    writeFileSync(join(dir, 'forbidden.txt'), 'a non-builder wrote this\n')
    git('add', '-A')
    git('commit', '-q', '-m', 'forbidden')
    const forbidden = git('rev-parse', 'HEAD')

    const result = revokeForbiddenCommit({
      allowCommit: false,
      workspace: dir,
      commitHash: forbidden,
      baseCommit: base,
    })

    assert.equal(result.commitHash, null)
    assert.match(String(result.violation), new RegExp(forbidden))
    // The point of the rewind: the forbidden commit is no longer reachable from the worktree, and the
    // file it added is gone. Dropping the hash from evidence without this would report a clean tree.
    assert.equal(git('rev-parse', 'HEAD'), base)
    assert.equal(git('status', '--porcelain'), '')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
