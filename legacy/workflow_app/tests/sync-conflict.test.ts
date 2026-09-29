import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { conflictCopies, conflictCopyBaseName, isConflictCopyName } from '@/lib/git/sync-conflict'

// ---------------------------------------------------------------------------
// SYNC CONFLICT COPIES — the debris of a cloud-sync client reconciling a file that changed mid-upload.
//
// Measured 2026-09-18: 554 of them in `.next` after one build, and ONE in the repository itself
// (`contact-export/contacts-export 2.json`). The cause turned out not to be iCloud, which is what the
// captain assumed: `~/Library/CloudStorage/OneDrive-Personal/` holds both `Documents` and `Desktop`,
// the signature of OneDrive Known Folder Move. So the noise had a name and a fix, and `.next` was not
// the only place it landed.
//
// THIS FENCE EARNED ITS KEEP ON ITS FIRST RUN. The rule was "space + number + extension", which also
// matches `CHANGELOG 2026.md` — so `pnpm health --fix` would have deleted a person's file. A copy is
// only debris when the counter is SMALL and the ORIGINAL sits beside it, and both are asserted below.
// ---------------------------------------------------------------------------

test('sync-conflict: a small numbered sibling beside its original is debris', () => {
  const files = ['routes.d.ts', 'routes.d 2.ts', 'package.json', 'package 3.json', 'only.ts']
  assert.deepEqual(conflictCopies(files), ['package 3.json', 'routes.d 2.ts'])
})

test('sync-conflict: a FOUR-DIGIT year is not a counter, so a dated document is never debris', () => {
  // The data-loss case this fence caught: real documents a person named, each beside its undated twin.
  const files = ['CHANGELOG.md', 'CHANGELOG 2026.md', 'Budget.xlsx', 'Budget 2026.xlsx']
  assert.deepEqual(conflictCopies(files), [])
  assert.equal(isConflictCopyName('CHANGELOG 2026.md'), false)
  assert.equal(conflictCopyBaseName('CHANGELOG 2026.md'), null)
})

test('sync-conflict: a numbered sibling with NO original beside it is left alone', () => {
  // Without the sibling test the rule is a guess. `Chapter 2.md` may be the only chapter there is.
  assert.deepEqual(conflictCopies(['Chapter 2.md', 'notes.ts']), [])
  assert.deepEqual(conflictCopies(['src/a 2.ts']), [])
})

test('sync-conflict: judgement never crosses a directory boundary', () => {
  assert.deepEqual(conflictCopies(['src/a.ts', 'other/a 2.ts']), [])
  assert.deepEqual(conflictCopies(['src/a.ts', 'src/a 2.ts']), ['src/a 2.ts'])
})

test('sync-conflict: the cleaner scans the SOURCE tree, where the copy actually hid', () => {
  // A source-level guard, because the miss was structural: the old scan walked only `.next`, so the
  // `contact-export 2.json` sitting in the repository was never reported by a tool whose job is debris.
  const script = readFileSync(new URL('../../../scripts/sprint-cleanup.ts', import.meta.url), 'utf8')
  assert.equal(/walkFiles\(root, \[\], SYNC_SKIP\)/.test(script), true, 'repo-wide conflict scan missing')
  assert.equal(/SYNC_SKIP/.test(script), true, 'the scan must skip .git and node_modules')
  assert.equal(/from '\.\.\/lib\/git\/sync-conflict'/.test(script), true, 'predicate not shared with the fence')
})

test('sync-conflict: the cleaner scans the SOURCE tree, where the copy actually hid', () => {
  // A source-level guard, because the miss was structural: the old scan walked only `.next`, so the
  // `contact-export 2.json` sitting in the repository was never reported by a tool whose job is debris.
  const script = readFileSync(new URL('../../../scripts/sprint-cleanup.ts', import.meta.url), 'utf8')
  assert.equal(/walkFiles\(root, \[\], SYNC_SKIP\)/.test(script), true, 'repo-wide conflict scan missing')
  assert.equal(/SYNC_SKIP/.test(script), true, 'the scan must skip .git and node_modules')
  assert.equal(/from '\.\.\/lib\/git\/sync-conflict'/.test(script), true, 'predicate not shared with the fence')
})
