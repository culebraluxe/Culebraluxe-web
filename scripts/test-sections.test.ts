import assert from 'node:assert/strict'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

import { SECTIONS, sectionForFile, sectionReport, sectionsForPaths } from './test-sections'

// ---------------------------------------------------------------------------
// THE TAXONOMY GATE.
//
// 385 test files, ~3,300 tests. The captain asked for APP and FORGE to be separable, and by section so
// we stop killing time. The only way that stays true is if EVERY test file is classified: an
// unclassified file is a file no section runs, i.e. a test nobody will ever run again. These run in
// pnpm test:harness, so a new test file that matches no rule stops the line until it is placed.
//
// THE PORT CHANGED THE TREE, NOT THE RULE (4cf98110). The 385 TypeScript suite files are gone; the tests
// are 168 files that hold their own tests — 149 Rust files with `#[cfg(test)]` beside the code they
// check, 8 Rust integration tests, and the scripts that survived. So the floors below are lower and the
// sections that named a deleted tree say so (`historical: true`, with the Rust crate that owns the
// capability now). What is checked did not weaken: nothing may be unclassified, and a section may not be
// empty without saying it is historical.
// ---------------------------------------------------------------------------

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

test('every test file in the repo belongs to exactly one section', () => {
  const report = sectionReport(root)
  assert.deepEqual(
    report.unclassified,
    [],
    'a test file matched no rule: add one in scripts/test-sections.ts (or an EXPLICIT entry with its reason)',
  )
  // 168 measured 2026-09-28. The floor is a tripwire against a walker that stops finding files — the same
  // failure mode the old one had, looking in four deleted trees and finding ten files.
  assert.ok(report.total > 100, `expected the real test tree, saw ${report.total} files`)
})

test('the APP and FORGE halves are both real, and nothing is in neither', () => {
  const report = sectionReport(root)
  const byArea = SECTIONS.reduce<Record<string, number>>((acc, s) => {
    acc[s.area] = (acc[s.area] ?? 0) + (report.bySection[s.name]?.length ?? 0)
    return acc
  }, {})
  // Measured 2026-09-28: APP 113 (ui 41, core 72), FORGE 43, HARNESS 12. The floors leave room for a story
  // to be deleted without the gate crying wolf, while still failing if a half goes hollow.
  assert.ok((byArea.APP ?? 0) > 25, `APP should hold the app tests, saw ${byArea.APP ?? 0}`)
  assert.ok((byArea.FORGE ?? 0) > 25, `FORGE should hold the engine tests, saw ${byArea.FORGE ?? 0}`)
  assert.equal(
    (byArea.APP ?? 0) + (byArea.FORGE ?? 0) + (byArea.HARNESS ?? 0),
    report.total,
    'every file must land in one of the three areas',
  )
})

test('every section is populated, or says out loud that its tree is gone', () => {
  const report = sectionReport(root)
  for (const section of SECTIONS) {
    const files = report.bySection[section.name]?.length ?? 0
    if (files === 0) {
      assert.ok(
        section.historical === true,
        `${section.name} is empty — either it is not a section, or the rule that fills it is wrong`,
      )
      assert.ok(
        (section.now ?? '').length > 10,
        `${section.name} is historical: say which Rust crate owns the capability now`,
      )
    }
    assert.ok(section.about.length > 20, `${section.name} must say what it is for`)
  }
  // At least one section must be populated, or the taxonomy has stopped describing this repository.
  assert.ok(
    SECTIONS.some((section) => (report.bySection[section.name]?.length ?? 0) > 0),
    'every section is empty: the rules and the tree have come apart',
  )
})

test('sectionForFile: a fence path keeps its section even after the story is long done', () => {
  // The reason this is a mapping and not a move: these exact paths are inside stories' frozen proofs.
  assert.equal(sectionForFile('legacy/workflow_app/tests/claim-clock.test.ts'), 'forge-engine')
  assert.equal(sectionForFile('legacy/workflow_app/tests/qa-disposition-vocab.test.ts'), 'forge-verify')
  assert.equal(sectionForFile('agent-runtime/lead-decision.test.ts'), 'forge-runtime')
  assert.equal(sectionForFile('scripts/protected-files.test.ts'), 'harness')
  assert.equal(sectionForFile('legacy/workflow_app/tests/clients-pagination.test.ts'), 'app-crm')
  assert.equal(sectionForFile('legacy/workflow_app/tests/auth-boundary.test.ts'), 'app-identity')
})

test('sectionForFile: the Rust tree is classified by the crate a person actually runs', () => {
  // The crate is the unit of the command (`cargo test -p forge`), so the crate is the section.
  assert.equal(sectionForFile('rust/forge/src/scope_manifest.rs'), 'forge-engine')
  assert.equal(sectionForFile('rust/cli/src/forge/manifest.rs'), 'forge-engine')
  assert.equal(sectionForFile('rust/core/workflow/src/types.rs'), 'forge-engine')
  assert.equal(sectionForFile('rust/server/src/api/engine.rs'), 'app-core')
  assert.equal(sectionForFile('rust/ui/src/update.rs'), 'app-portal')
  assert.equal(sectionForFile('rust/cli/src/main.rs'), 'harness')
})

test('sectionsForPaths: a change runs the sections it can affect, and says so by area', () => {
  assert.deepEqual(sectionsForPaths(root, ['legacy/workflow_app/forge/forge-executor.ts']), [
    'forge-engine',
    'forge-verify',
  ])
  assert.deepEqual(sectionsForPaths(root, ['agent-runtime/invoker.ts']), ['forge-runtime'])
  assert.deepEqual(sectionsForPaths(root, ['components/portal/x.tsx']), ['app-portal'])
  assert.deepEqual(sectionsForPaths(root, ['legacy/workflow_app/tests/claim-clock.test.ts']), ['forge-engine'])
  assert.deepEqual(sectionsForPaths(root, ['docs/agent/MEMORY.md']), [], 'docs change no tests')
  // The same answer the report gives, from the same rules: a change to a Rust crate runs that crate.
  assert.deepEqual(sectionsForPaths(root, ['rust/forge/src/engine/runner.rs']), ['forge-engine'])
  assert.deepEqual(sectionsForPaths(root, ['rust/core/db/src/pool.rs']), ['app-core'])
  assert.deepEqual(sectionsForPaths(root, ['rust/Cargo.lock']), ['app-core', 'harness'])
})
