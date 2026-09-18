import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  MACHINE_OWNED_BRANCH,
  branchClassOf,
  branchIsDeletable,
  branchMayBeDeleted,
} from '../../lib/git/branch-hygiene'

// ---------------------------------------------------------------------------
// ENG-FORGE-BRANCH-HYGIENE-01 — the rules that let a cleaner DELETE a branch.
//
// Written after the noise was measured on 2026-09-18: 106 local and 62 remote branches had piled up,
// and the counter-wipe could not see them because it judged a branch by ANCESTRY (`git branch
// --merged main`) while work here lands by rebase and cherry-pick. A rebase-landed branch is not an
// ancestor of main, so "not merged" was the tool's answer forever, and `git branch -d` refuses the
// same branches for the same wrong reason — which is why the cleaner deletes by force and must have
// PROOF first.
//
// Every case here is one of the four things a branch can be, plus the ownership rule that keeps a
// person's branch out of the machine's hands.
// ---------------------------------------------------------------------------

test('branch-hygiene: a rebase-landed branch is LANDED even though it is not an ancestor of main', () => {
  // This is the exact shape that defeated the ancestry test: patches applied, tip not an ancestor.
  assert.equal(branchClassOf({ name: 'agent/doc-08/e1', unappliedPatches: 0, inWorktree: false }), 'landed')
  assert.equal(branchIsDeletable('landed'), true)
})

test('branch-hygiene: a branch carrying patches main has not seen is LEFT ALONE', () => {
  assert.equal(
    branchClassOf({ name: 'agent/eng-forge-v5-11/c67cdae5', unappliedPatches: 1, inWorktree: false }),
    'unlanded',
  )
  assert.equal(branchIsDeletable('unlanded'), false)
  assert.equal(branchMayBeDeleted('agent/eng-forge-v5-11/c67cdae5', 3, false), false)
})

test('branch-hygiene: test fixtures are deletable even with unapplied patches (a rehearsal is not work)', () => {
  for (const name of [
    'agent/eng-forge-smoke-01/78b611de',
    'agent/eng-forge-opencode-dogfood-01/00e9cf64',
    'forge/opencode-dogfood-01',
    'agent/integration-chk-mt4k476t-43d64f/dry-run',
  ]) {
    assert.equal(branchClassOf({ name, unappliedPatches: 2, inWorktree: false }), 'scaffolding', name)
    assert.equal(branchIsDeletable('scaffolding'), true, name)
  }
})

test('branch-hygiene: a branch checked out in a worktree is NEVER deleted, landed or not', () => {
  // The live case is `cmd-01`: a second checkout on this machine. Deleting its branch would yank a
  // workspace out from under whoever is using it.
  assert.equal(branchClassOf({ name: 'cmd-01', unappliedPatches: 0, inWorktree: true }), 'worktree')
  assert.equal(branchMayBeDeleted('cmd-01', 0, true), false)
})

test('branch-hygiene: landed is not enough — a branch a PERSON named is not ours to delete', () => {
  // Measured on the remote: 29 branches were provably landed, and most were `feat/*`, `v0/*`,
  // `demo-lockdown/*`. Content-safe to remove, but a remote one can still carry a live preview
  // deployment, and the name says a person made it. Reported, never deleted.
  for (const name of ['feat/marketing-next5', 'v0/some-preview', 'demo-lockdown/portal-12-storyboard-cockpit']) {
    assert.equal(branchMayBeDeleted(name, 0, false), false, name)
  }
  assert.equal(MACHINE_OWNED_BRANCH.test('feat/marketing-next5'), false)
  assert.equal(MACHINE_OWNED_BRANCH.test('agent/eng-22/e1'), true)
  assert.equal(branchMayBeDeleted('agent/eng-22/e1', 0, false), true)
})

test('branch-hygiene: the wrong test is not still sitting in the cleaner', () => {
  // A source-level guard, because this bug was invisible in behaviour: the tool reported "no merged
  // branches" and looked healthy. Ancestry (`--merged`) must not come back as a CALL, and deletion
  // must not drop to `git branch -d`, which cannot delete the rebase-landed branches this exists for.
  //
  // Matched as executable call forms, not bare phrases: the file documents the old test by name in a
  // comment (why it was wrong is the useful part), and a guard that trips over its own documentation
  // gets deleted by the next person instead of the bug.
  const script = readFileSync(new URL('../../scripts/sprint-cleanup.ts', import.meta.url), 'utf8')
  assert.equal(/sh\(`git branch --merged/.test(script), false, 'ancestry test reintroduced as a call')
  assert.equal(/`git branch -d /.test(script), false, 'git branch -d cannot see rebase-landed work')
  assert.equal(/`git cherry /.test(script), true, 'patch-based land test missing')
  assert.equal(/`git push origin --delete/.test(script), true, 'remote sweep missing')
})
