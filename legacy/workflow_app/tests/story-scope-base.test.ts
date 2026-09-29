import test from 'node:test'
import assert from 'node:assert/strict'

import { storyScopeBase } from '@/legacy/workflow_app/forge/story-scope-base'

// ---------------------------------------------------------------------------
// ENG-FORGE-SCOPE-BASE — the scope gate diffs against the STORY's base, never a remote ref that lags.
//
// Measured 2026-09-16 (ENG-FORGE-QA-CONSISTENCY-01): the gate diffed from `origin/main`, which under a
// deferred-publish sprint sits 17 commits behind, so the "candidate diff" was every file touched that night
// and the smith's completed work was held as out-of-scope. The story's base is the parent of its own first
// commit — a fact the machine already records.
// ---------------------------------------------------------------------------

const A = 'a'.repeat(40)
const B = 'b'.repeat(40)
const C = 'c'.repeat(40)

test('the base is the parent of the EARLIEST commit the story made', () => {
  // newest-first, as every reader supplies them
  const commits = [C, B, A]
  const parents: Record<string, string> = { [A]: 'f'.repeat(40) }
  assert.equal(
    storyScopeBase(commits, (sha) => parents[sha] ?? null),
    'f'.repeat(40),
    'A is the earliest commit, so its parent is the base',
  )
})

test('a story that has made no commit has no base — never a guess', () => {
  assert.equal(storyScopeBase([], () => 'x'.repeat(40)), null)
  assert.equal(storyScopeBase([null, undefined, '  '], () => 'x'.repeat(40)), null)
  assert.equal(
    storyScopeBase(['not-a-sha', '12ab'], () => 'x'.repeat(40)),
    null,
    'a non-sha is not a commit',
  )
})

test('an unreadable parent is null rather than a fabricated base', () => {
  assert.equal(storyScopeBase([B], () => null), null, 'a root commit has no parent')
})

test('the sha is normalized so callers cannot disagree about case or spacing', () => {
  const upper = 'D'.repeat(40)
  assert.equal(storyScopeBase([`  ${upper}  `], () => 'e'.repeat(40)), 'e'.repeat(40))
})
